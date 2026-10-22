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
    /// presenter cannot read one. UIKit's presenter keeps a native button's
    /// until a batch changes it (`ControlHost.face`); a custom button's face
    /// is read from its child views, whose ops do not name the button, so
    /// it is read each time.
    #if os(macOS)
    var face: ButtonFace? { presenter?.buttonFace?(id) }
    #else
    var face: ButtonFace? {
        guard let presenter, presenter.buttonFace != nil else { return nil }
        return isNativeButton ? presenter.controls.face(id) : presenter.buttonFace?(id)
    }
    #endif

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
    /// with `role="checkbox"` as a checkbox (habits F16). A native module's
    /// box takes these roles as a box does.
    /// On the Mac a menu's row (`menuitem`, `menuitemcheckbox`,
    /// `menuitemradio`) is one too, as AppKit's menu item is.
    var actsAsButton: Bool {
        #if os(macOS)
        let menuItem = isMenuItem
        #else
        let menuItem = false
        #endif
        return kind == "button" || (kind == "view" || kind == "native") && (["button", "link", "checkbox", "radio", "switch"].contains(props["accessibilityRole"] ?? "") || menuItem)
    }
    /// ARIA's menu item roles, each AppKit's `AXMenuItem` (Core-AAM).
    static let menuItemRoles: Set<String> = ["menuitem", "menuitemcheckbox", "menuitemradio"]
    var isMenuItem: Bool { Self.menuItemRoles.contains(props["accessibilityRole"] ?? "") }
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
        // A checked `menuitemcheckbox` or `menuitemradio` carries the check
        // mark AppKit's checked menu item does (Core-AAM, as WebKit serves it).
        case "AXMenuItemMarkChar":
            return ["menuitemcheckbox", "menuitemradio"].contains(props["accessibilityRole"] ?? "") && props["accessibilityChecked"] == "true" ? "✓" : nil
        default: return nil
        }
    }
    static let ariaAttributes = ["AXEnabled", "AXInvalid", "AXHasPopup", "AXPopupValue", "AXARIACurrent", "AXMenuItemMarkChar"]
    /// ARIA's live-region roles with an implicit `aria-live` (WAI-ARIA 1.2):
    /// `status` is polite, `alert` assertive.
    static let liveRoles = ["status": "polite", "alert": "assertive"]
    var liveRole: String? { props["accessibilityRole"].flatMap { Self.liveRoles[$0] == nil ? nil : $0 } }
    #if os(macOS)
    /// The ARIA roles WebKit serves as AppKit's group with the role as its
    /// subrole (Core-AAM): the live regions and the dialogs, a `dialog`
    /// element's implicit role among them (HTML-AAM), and a `region`, a
    /// landmark only with a name (without one it is generic).
    static let groupSubroles = ["status": "AXApplicationStatus", "alert": "AXApplicationAlert",
                                "dialog": "AXApplicationDialog", "alertdialog": "AXApplicationAlertDialog",
                                "region": "AXLandmarkRegion"]
    var groupSubrole: String? {
        let role = props["accessibilityRole"] ?? (props["semanticTag"] == "dialog" ? "dialog" : nil)
        if role == "region", authoredLabel == nil { return nil }
        return role.flatMap { Self.groupSubroles[$0] }
    }
    /// The ARIA roles that make a box an AppKit container of its own
    /// (Core-AAM): a `menu` is `AXMenu`, as WebKit serves one, so VoiceOver
    /// says "menu" as it does for AppKit's.
    static let containerRoles: [String: NSAccessibility.Role] = ["group": .group, "radiogroup": .radioGroup, "menu": .menu, "menubar": .menuBar]
    /// What a dialog says as it opens: its name, its role ("dialog") and its
    /// description, in VoiceOver's order for an element; nil with neither a
    /// name nor a description.
    var dialogWords: String? {
        let name = authoredLabel, description = accessibleDescription
        guard name != nil || description != nil else { return nil }
        return [name, accessibilityRoleDescription(), description].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: ", ")
    }
    /// A pressable's AppKit role when it is no toggle: a link, a menu item, else a button.
    var buttonRole: NSAccessibility.Role { props["accessibilityRole"] == "link" ? .link : isMenuItem ? .menuItem : .button }
    var isMenu: Bool { ["menu", "menubar"].contains(props["accessibilityRole"] ?? "") }
    /// The `menu` or `menubar` this node is in, the nearest.
    var owningMenu: NodeView? {
        var next = presenter?.menus.parent(of: self) ?? superview
        while let view = next {
            if let node = view as? NodeView, node.isMenu { return node }
            next = presenter?.menus.parent(of: view) ?? view.superview
        }
        return nil
    }
    /// The row of this menu that holds the keyboard focus: what AppKit's
    /// menu has highlighted, its selected child.
    var focusedMenuRow: NodeView? {
        guard let presenter, let row = presenter.keyTarget(window?.firstResponder), row.isMenuItem, row.owningMenu === self else { return nil }
        return row
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
        // UIKit has no property for `aria-required`, `aria-invalid`, `aria-haspopup` or `aria-current` (a button's current is its selected trait, NodeViewIOS).
        let target: UIView = field ?? presenter?.controls.controls[id] ?? textArea as UIView? ?? self
        // Only on change: a set is kept in UIKit's associated object table.
        if target.accessibilityTraits.contains(.notEnabled) != (disabled || ariaUnavailable) { target.accessibilityTraits.formSymmetricDifference(.notEnabled) }
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
        guard let field = controlView as? Field, field.numberOwner != nil else { return nil }
        return field
    }
    /// The session's editor, which tells the field's node what the window's does not (`FieldEditor`).
    override func fieldEditor(for controlView: NSView) -> NSTextView? {
        // Through its delegate, the node, wherever the field is: a prompt's
        // alert holds it in its accessory view (AlertsMac.swift).
        let node = (controlView as? NSTextField)?.delegate as? NodeView ?? controlView.superview as? NodeView
        return node?.presenter?.fieldEditor ?? super.fieldEditor(for: controlView)
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
            + ((controlView as? NSTextField)?.comboOwner != nil ? [.expanded] : [])
    }
    override func accessibilityAttributeValue(_ attribute: NSAccessibility.Attribute) -> Any? {
        if let value = (controlView as? NSTextField)?.comboboxAttribute(attribute) { return value }
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
    /// traits have it: a pressable is a button — a link, checkbox, radio,
    /// switch or menu item when its role says so — a labelled image (or
    /// `role="img"`, an svg's) an image, and a `group`, `radiogroup`, `menu`
    /// or `menubar` a container of its children
    /// (`containerRoles`). Headings are paragraphs (`updateTextAccessibility`); names
    /// come from `syncAccessibility`.
    func updateRoleAccessibility() {
        let role = props["accessibilityRole"]
        if actsAsButton {
            setAccessibilityElement(true)
            if let checked = checkedRole { setAccessibilityChecked(checked.role, checked.checked) }
            else { setAccessibilityToggle(pressedState, else: buttonRole) }
            // A link's `AXURL` is its `href`, as a run's is (X78).
            let url = role == "link" ? props["href"].flatMap { URL(string: $0) } : nil
            if accessibilityURL() != url { setAccessibilityURL(url) }
            setAccessibilitySelected(props["accessibilitySelected"] == "true")
            if let expanded = props["accessibilityExpanded"] { setAccessibilityExpanded(expanded == "true") }
        } else if kind == "image" || role == "img" {
            let labelled = authoredLabel != nil
            setAccessibilityElement(labelled)
            setAccessibilityRole(labelled ? .image : nil)
        } else if let container = role.flatMap({ Self.containerRoles[$0] }) {
            setAccessibilityElement(true)
            if accessibilityRole() != container { setAccessibilityRole(container) }
            if accessibilitySubrole() != nil { setAccessibilitySubrole(nil) }
        } else if let subrole = groupSubrole.map(NSAccessibility.Subrole.init(rawValue:)) {
            // A live-region, dialog or region role as WebKit serves it (Core-AAM): a
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
    /// A dialog or alert dialog says AppKit's "dialog", the role description
    /// of a dialog window and an alert's, where AppKit would derive "group"
    /// from the subrole and WebKit says "web dialog". It reads the host's
    /// own subrole: with the `accessibilitySubrole()` getter called here,
    /// another process (VoiceOver) was served no role description for a
    /// group or a menu (measured), so a labelled group read without "group".
    package override func accessibilityRoleDescription() -> String? {
        switch groupSubrole {
        case "AXApplicationDialog", "AXApplicationAlertDialog": return NSAccessibility.Role.window.description(with: .dialog)
        // Chromium's and WebKit's word for a named region (Core-AAM), which
        // AppKit has none of: it would say "group".
        case "AXLandmarkRegion": return "region"
        default: return super.accessibilityRoleDescription()
        }
    }
    /// VoiceOver's show menu (AXShowMenu, VO-⇧-M) is the ContextMenu key's
    /// request at this node (X84), as Chrome sends both through Blink's
    /// `ShowNonLocatedContextMenu`: the `contextmenu`, then the
    /// `contextPopover`'s menu, of the first node from this one up that
    /// answers (`contextTarget`; the event bubbles), at this one's drawn middle.
    package override func accessibilityPerformShowMenu() -> Bool {
        guard let target = showMenuTarget, let presenter, let point = presenter.contextPoint(self) else { return false }
        return presenter.contextMenu(target, at: point, from: self)
    }
    /// AppKit offers every action whose method the class implements; show
    /// menu only where something answers it, as AppKit offers it only on a
    /// view with a `menu` (no node has one).
    package override func isAccessibilitySelectorAllowed(_ selector: Selector) -> Bool {
        selector == #selector(accessibilityPerformShowMenu) ? showMenuTarget != nil : super.isAccessibilitySelectorAllowed(selector)
    }
    package override func accessibilityChildrenInNavigationOrder() -> [any NSAccessibilityElementProtocol]? { treeNavigationOrder }
    /// A menu's selected child is the row that holds the focus, and that row
    /// is selected, as AppKit's menu serves its highlighted item (measured:
    /// `AXSelectedChildren` and `AXSelected` follow the highlight).
    package override func accessibilitySelectedChildren() -> [Any]? {
        guard isMenu, let presenter else { return super.accessibilitySelectedChildren() }
        return focusedMenuRow.map { [presenter.keyView(of: $0)] } ?? []
    }
    package override func isAccessibilitySelected() -> Bool {
        guard isMenuItem, let menu = owningMenu else { return super.isAccessibilitySelected() }
        return menu.focusedMenuRow === self
    }
    private var showMenuTarget: NodeView? {
        guard let presenter, presenter.views[id] === self, !formDisabled, !inert, field == nil, textArea == nil,
              let target = presenter.contextTarget(self), !target.formDisabled, !target.authoredInert else { return nil }
        return target
    }
    /// The host's own record of the flag, for code that serves an
    /// accessibility request: AppKit drops its reply to another process
    /// (VoiceOver) when the code serving it calls `isAccessibilityHidden()`,
    /// and the client is told the attribute is unsupported.
    package override func setAccessibilityHidden(_ hidden: Bool) {
        accessibilityHiddenWritten = hidden
        super.setAccessibilityHidden(hidden)
    }
}

extension NSView {
    /// `children` without the nodes the host hid from accessibility
    /// (`setAccessibilityHidden`: inert, behind a modal dialog, `aria-hidden`
    /// above). AppKit serves such a view to another process all the same
    /// (measured: an `inert` page's button was VoiceOver's to land on), where
    /// a browser leaves an inert subtree out of its accessibility tree. A
    /// hidden view that is no element hoists nothing: its own children are
    /// none (`NodeView.accessibilityChildren`).
    func exposed(_ children: [Any]?) -> [Any]? {
        children?.filter { ($0 as? NodeView)?.accessibilityHiddenWritten != true }
    }
    /// VoiceOver's order, `AXChildrenInNavigationOrder`, as the children
    /// `AXChildren` serves: the order the author wrote, as a browser serves
    /// the DOM's. AppKit's own sorts them by frame, so a chip laid over a
    /// textarea's top-left corner was read before the textarea.
    /// `NSAccessibilityElement` (a paragraph's runs) does not formally adopt
    /// the protocol the property is typed with; the cast is only nominal.
    var treeNavigationOrder: [any NSAccessibilityElementProtocol]? {
        accessibilityChildren()?.map { unsafeBitCast($0 as AnyObject, to: (any NSAccessibilityElementProtocol).self) }
    }
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
        if accessibilityTraits.contains(.toggleButton) != (pressed != nil) { accessibilityTraits.formSymmetricDifference(.toggleButton) }
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
            let current = window.firstResponder, click = pressMount(node, in: window)
            if (current as? NodeView)?.returnsPointerFocusToCanvas != true { autofocusProcessed.insert(ObjectIdentifier(node)) }
            guard click != nil || current == nil || current === window || current === window.contentView || current === viewport || current === session?.view || (current as? NodeView)?.canvasInput != nil else { continue }
            // Blocked autofocus stays pending until the pointer hands focus back.
            autofocusProcessed.insert(ObjectIdentifier(node))
            let target = keyView(of: node)
            if target.acceptsFirstResponder, window.makeFirstResponder(target), let click,
               let editor = node.textArea ?? (node.field?.currentEditor() as? NSTextView) {
                editor.setSelectedRange(NSRange(location: Self.caret(in: editor, at: click), length: 0))
            }
            #else
            // Any focus a node holds stays, however it came (Charlie, 2026-10-17), except: the session's own
            // view holding it for its shortcuts (ShortcutsIOS), a surface control's from its touch, which the
            // author did not ask for, and the focus a press left on its control while that press's work runs,
            // which an autofocus the work mounts takes (PressHandoffIOS).
            func yields(_ view: UIView) -> Bool { (view as? NodeView)?.focusedBySurfaceTouch == true || pressHandsOff(view, to: node) }
            func hasFocus(_ view: UIView) -> Bool { (view.isFirstResponder && (view as? NodeView)?.canvasInput == nil && !yields(view) && !(view is ExactView)) || view.subviews.contains(where: hasFocus) }
            guard let window = node.window else { continue }
            autofocusProcessed.insert(ObjectIdentifier(node))
            guard !hasFocus(window) else { continue }
            let target: UIResponder = node.textArea ?? node.field ?? node, caret = pressCaret(in: node)
            autofocused(node) // the app's focus now, as Tab's and focus(id)'s
            if target.becomeFirstResponder() {
                if let (input, at) = caret { input.selectedTextRange = input.textRange(from: at, to: at) }
                autofocusedThisTurn = node
                DispatchQueue.main.async { [weak self] in self?.autofocusedThisTurn = nil }
            }
            #endif
        }
        #if os(macOS)
        // A `menu` says it opened and closed as AppKit's menu does
        // (AXMenuOpened, AXMenuClosed, on the menu; measured), so VoiceOver
        // says "menu" and, after, leaves the menu. One taken out of the
        // window said so as it left (`viewWillMove(toWindow:)`, FocusMac.swift).
        let shown = chrome.ids("role:menu").compactMap { views[$0] }.filter(\.accessibilityVisible)
        for menu in shown where !shownMenus.contains(where: { $0 === menu }) {
            NSAccessibility.post(element: menu, notification: .init(rawValue: "AXMenuOpened"))
            if menu.focusedMenuRow != nil {
                menusHeldFocus.add(menu)
                NSAccessibility.post(element: menu, notification: .selectedChildrenChanged)
            } else { menusHeldFocus.remove(menu) }
        }
        for menu in shownMenus where !shown.contains(where: { $0 === menu }) { menuClosed(menu) }
        shownMenus = shown
        if macAccessibilityModal.map(ObjectIdentifier.init) != toldModal { scheduleLanding() }
        #else
        syncModal()
        #endif
    }
    #if os(macOS)
    /// A shown `menu` closed: it says so while it is still an element to
    /// post on, and VoiceOver is sent back to the focus if a row of it held
    /// the focus. One whose rows never had it (opened by VO-Space and closed
    /// by Escape) leaves VoiceOver where it was, as AppKit's `NSMenu`, which
    /// never takes the focus, posts nothing after AXMenuClosed (measured).
    func menuClosed(_ menu: NodeView) {
        if menu.window != nil { NSAccessibility.post(element: menu, notification: .init(rawValue: "AXMenuClosed")) }
        if menusHeldFocus.contains(menu) {
            menusHeldFocus.remove(menu)
            landing.focusReturns = true
            scheduleLanding()
        }
    }
    /// VoiceOver is sent where the keyboard focus is once the commit that
    /// changed the modal or closed a menu has run its commands, so the
    /// landing is the element an opening commit's `focus()` or the focus
    /// given back chose, not the one the batch left.
    func scheduleLanding() {
        guard !landing.scheduled else { return }
        landing.scheduled = true
        DispatchQueue.main.async { [weak self] in self?.tellLanding() }
    }
    /// AppKit tells a dialog or a menu-like list that takes the focus by the
    /// focus: the focused element's AXFocusedUIElementChanged, last
    /// (measured with a sheet, a panel, an `NSPopover` and a child window
    /// whose field or row is the first responder; none posts a layout
    /// change). A changed modal also posts its one layout change
    /// (LLP 1080.003 D3), naming the same element: the focus when it is in
    /// the modal (posted on the modal), else the modal; as a modal goes,
    /// the focus given back, or the trigger of a popover that closed with
    /// the focus where it was, which VoiceOver returns to from an `NSMenu`
    /// while the focus stays put. The focus change is the host's only when AppKit did not post
    /// it: AppKit posts one as a view becomes the first responder of the
    /// active application's key window (measured), and a second for the
    /// same element is not AppKit's. A dialog drawn in the page is then
    /// named (`nameDialog`).
    func tellLanding() {
        landing.scheduled = false
        let modal = macAccessibilityModal
        let focus = focusedNode.map { keyView(of: $0) }
        let (focusReturns, invoker, invokerFocus) = (landing.focusReturns, landing.invoker, landing.invokerFocus)
        landing.focusReturns = false
        landing.invoker = nil
        landing.invokerFocus = nil
        // A first responder that changed since the last landing in the
        // active application's key window: AppKit told it (measured; it
        // posts nothing for an inactive application's window).
        let toldByAppKit = focus.map { $0 !== landing.toldFocus && $0.window?.isKeyWindow == true && NSApp.isActive } ?? false
        defer { landing.toldFocus = focus }
        if modal.map(ObjectIdentifier.init) != toldModal {
            toldModal = modal.map(ObjectIdentifier.init)
            // The trigger only while the focus is the one the popover closed
            // with: a later command that moved it is where VoiceOver goes.
            var trigger: NSView?
            if let invoker, views[invoker.id] === invoker, invoker.accessibilityVisible,
               viewport.window.flatMap(menus.focusOwner) === invokerFocus,
               modal.map({ menus.contains($0, invoker) }) ?? true { trigger = keyView(of: invoker) }
            let target = trigger ?? modal.map(modalLanding) ?? focus
            // The focus in the modal is posted from the modal. Posted on the
            // page's scroll view, VoiceOver went to that view's child holding
            // the element, the modal, whenever the modal had just hidden the
            // trigger its cursor was on (VO-Space): it said "<name>, dialog"
            // and never the field that had the focus (measured). A modal
            // that is no element (no role) hears nothing posted on it, and
            // the scroll view serves its elements as its own children
            // (`PageScrollView`): the change is the scroll view's.
            let changed: NSView = (trigger == nil && focus != nil && target === focus ? modal : nil)
                .flatMap { $0.isAccessibilityElement() ? $0 : nil } ?? viewport
            NSAccessibility.post(element: changed, notification: .layoutChanged, userInfo: target.map { [.uiElements: [$0]] })
            if let focus, target === focus, !toldByAppKit { NSAccessibility.post(element: focus, notification: .focusedUIElementChanged) }
            nameDialog(modal, landedOn: target)
        } else if focusReturns, let focus, !toldByAppKit {
            NSAccessibility.post(element: focus, notification: .focusedUIElementChanged)
        }
    }
    /// How long after the landing a dialog drawn in the page is named.
    static let dialogNameDelay: TimeInterval = 0.5
    /// A dialog drawn in the page that became the modal is named after the
    /// focus in it is told: VoiceOver reads a sheet's or a panel's title as
    /// it appears and an `NSAlert`'s title and message, but a group in the
    /// page has no window to be read by. The words are queued at low
    /// priority, as a polite live region's are, so they follow the focused
    /// control's and do not cut it off, a moment after the landing:
    /// VoiceOver drops words that arrive before it has moved to a text
    /// field it was sent to from a trigger the modal hid (VO-Space on the
    /// font picker's trigger, measured: none of five read at once, the
    /// field and then the name read 0.3 s later). One that is no longer the
    /// modal by then is not named. A dialog VoiceOver landed on itself
    /// is read there; a sheet is AppKit's to read, and so is AppKit's
    /// popover: VoiceOver says "In <name>, dialog" as the focus enters one
    /// (measured), where a popover painted in the page is named here. It is
    /// named once while it stays up:
    /// one handed the modal back by a dialog closed over it is not named
    /// again, as an AppKit sheet is not when its child sheet ends.
    private func nameDialog(_ modal: NodeView?, landedOn target: NSView?) {
        landing.namedDialogs.removeAll { dialog in
            dialog.window == nil || dialog.isHiddenOrHasHiddenAncestor || leaving[dialog.id] != nil
                || (dialog.props["accessibilityModal"] != "true" && dialogs.active !== dialog)
        }
        guard let modal, ["AXApplicationDialog", "AXApplicationAlertDialog"].contains(modal.groupSubrole ?? ""),
              modal.window?.isSheet != true, !(modal.window.map(menus.hosts) ?? false), !landing.namedDialogs.contains(where: { $0 === modal }) else { return }
        landing.namedDialogs.append(modal)
        guard target !== modal, let words = modal.dialogWords else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.dialogNameDelay) { [weak self, weak modal] in
            guard let self, let modal, macAccessibilityModal === modal else { return }
            NSAccessibility.post(element: modal, notification: .announcementRequested,
                userInfo: [.announcement: words, .priority: NSAccessibilityPriorityLevel.low.rawValue])
        }
    }
    /// Where VoiceOver goes as a modal arrives: the keyboard focus when it
    /// is in the modal (a field it autofocused, the button `focus()` named),
    /// else the modal, so it reads the modal's name and role.
    func modalLanding(_ modal: NodeView) -> NSView {
        guard let focused = focusedNode, menus.contains(modal, focused) else { return modal }
        return keyView(of: focused)
    }
    var macModalAccessibilityChildren: [NodeView]? {
        guard let modal = macAccessibilityModal else { return nil }
        // In its sheet the dialog is that window's (LLP 1115.002 D7.2): the page behind it exposes nothing.
        if modal.window !== viewport.window { return [] }
        return [modal] + menus.presented(within: modal).filter { $0 !== modal && $0.window === viewport.window }
    }
    /// The frontmost exposed authored modal, or the dialog in HTML's top
    /// layer. The viewport serves it directly so AppKit cannot flatten a
    /// scroll or a native control beside it into the accessible page.
    /// It serves AppKit's requests, so it reads the host's own state and
    /// never AppKit's accessibility getters (`setAccessibilityHidden`).
    var macAccessibilityModal: NodeView? {
        func hidden(_ view: NSView) -> Bool { (view as? NodeView)?.accessibilityHiddenWritten == true }
        if let active = dialogs.active, active.accessibilityVisible,
           active.props["accessibilityElementsHidden"] != "true", !hidden(active),
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
                  !hidden(node), leaving[node.id] == nil else { return false }
            for view in sequence(first: node as NSView, next: \.superview) {
                if hidden(view) { return false }
                guard let parent = view.superview else { break }
                if parent.subviews.contains(where: { sibling in
                    guard sibling !== view, let cover = sibling as? NodeView,
                          !cover.cssVisibilityHidden, !cover.isHidden, !hidden(cover),
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

#if os(macOS)
/// What `Presenter.tellLanding` tells VoiceOver on the turn after a commit.
struct VoiceOverLanding {
    var scheduled = false
    /// A menu whose rows held the focus closed: the focus it left is told.
    var focusReturns = false
    /// The trigger of a popover that closed with the focus where it was, and
    /// that focus owner: VoiceOver goes to the trigger while it still is.
    var invoker: NodeView?
    weak var invokerFocus: NSView?
    /// The focus at the last landing: one changed since was told by AppKit.
    weak var toldFocus: NSView?
    /// The dialogs drawn in the page named as they opened that are still up.
    var namedDialogs: [NodeView] = []
}
#endif

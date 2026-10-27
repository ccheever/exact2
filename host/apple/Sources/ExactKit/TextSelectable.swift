// @ref LLP 1115 D8, LLP 1115.000 D11.6 — which text a reader can select:
// CSS `user-select`, its `auto` resolved as each platform resolves it.
#if os(macOS)
import AppKit
private typealias AncestorView = NSView
#else
import UIKit
private typealias AncestorView = UIView
#endif

extension NodeView {
    /// Whether this text takes part in selection. Walking up from the text:
    /// 1. the nearest authored `user-select` decides: `none` no; `text` and
    ///    `all` yes (`contain`, which the compiler refuses, as `text`);
    /// 2. else the text is a control's label, and not selectable, inside a
    ///    pressable (a button or link, a box with a press action, a `button`,
    ///    `link`, `tab`, `menuitem`, `option`, `checkbox`, `radio` or
    ///    `switch` role) or a native control.
    ///
    /// What is left is `auto` with nothing above it, and the platform's.
    /// On iOS and tvOS it is not selectable: a hand-built screen's text is
    /// a `UILabel`, which selects nothing, and only text its author made
    /// selectable is (LLP 1115.000 D11.6; `TextSelectionIOS.swift`). On the
    /// Mac it is AppKit's (LLP 1115 D8), where a control's or the window
    /// chrome's label is not selectable (an `NSTextField` label, an
    /// `NSButton` title, a toolbar, a tab, a table row's cell) and a
    /// document's text is (an `NSTextView`):
    /// 3. it is selectable inside an `article` (its own header too);
    /// 4. else it is chrome, not selectable, inside a `header`, `nav` or
    ///    `footer`, or a `toolbar`, `tablist`, `menubar`, `menu`, `listbox`
    ///    or `tree` role;
    /// 5. else it is content and selectable, as a plain list's rows are (a
    ///    transcript): a row that is pressable is a control by 2.
    /// ⌘A selects what this admits. The web host keeps the browser's own.
    var textSelectable: Bool {
        var document = false, chrome = false
        var current: AncestorView? = self
        while let view = current {
            // Through a presented route's logical parents (LLP 1123 D2)
            current = presenter?.eventParent(of: view) ?? view.superview
            guard let node = view as? NodeView else { continue }
            switch node.style["user_select"]?.string {
            case "none": return false
            case "text", "all", "contain": return true
            default: break
            }
            let role = node.props["accessibilityRole"] ?? ""
            if node.isControl { return false }
            let tag = node.props["semanticTag"] ?? ""
            if tag == "article" || role == "article" || role == "document" { document = true }
            if ["header", "nav", "footer"].contains(tag) || Self.chromeRoles.contains(role) { chrome = true }
        }
        #if os(macOS)
        return document || !chrome
        #else
        return false
        #endif
    }
    /// The outermost view from here up that says `user-select: all`: CSS UI
    /// 4 §6.1 selects its content atomically, so a selection that takes part
    /// of it takes all of it. An element inside another is part of the outer.
    var selectAllElement: NodeView? {
        var found: NodeView?, current: AncestorView? = self
        while let view = current {
            if let node = view as? NodeView, node.style["user_select"]?.string == "all" { found = node }
            current = view.superview
        }
        return found
    }
    private static let controlRoles: Set<String> = ["button", "link", "tab", "menuitem", "menuitemcheckbox", "menuitemradio",
                                                    "option", "checkbox", "radio", "switch", "treeitem", "slider", "spinbutton"]
    private static let chromeRoles: Set<String> = ["toolbar", "tablist", "menubar", "menu", "listbox", "tree"]
    /// A control: a node a press acts on, a button, or one in a control's role
    var isControl: Bool {
        pressable || isButton || isSurfaceControl || kind == "control" || Self.controlRoles.contains(props["accessibilityRole"] ?? "")
    }
    #if !os(macOS)
    /// Whether a press here does something: its own action, its link, or
    /// the popover or command a button names (the Mac's `pressable`).
    var pressable: Bool { handlers.contains("press") || defaultLink != nil || presenter?.invocation(of: self) != nil }
    #endif
    #if os(tvOS)
    // tvOS has no text selection (`TextSelectionIOS.swift` is iOS's).
    func touchedText() {}
    #endif
}

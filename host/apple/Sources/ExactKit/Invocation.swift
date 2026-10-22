#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// What pressing a button does to an element it names, as HTML's button
/// activation resolves it, for the commands Exact runs: the popover commands,
/// `show-modal` and `close`. A `commandfor` that names an element governs and
/// nothing falls back, even when its `command` cannot act on that element;
/// otherwise `popovertarget` acts by `popovertargetaction`. Keywords match
/// ASCII case-insensitively and untrimmed; an invalid `popovertargetaction`
/// toggles. (HTML: the button element's activation behaviour, and the popover
/// target attribute activation behaviour.)
struct Invocation {
    enum Action { case show, hide, toggle, showModal, close }
    let target: NodeView
    let action: Action

    /// Whether it can put its target up (`show`, `toggle`, `show-modal`)
    var opens: Bool { action != .hide && action != .close }
    /// Whether it opens a popover, as a header's menu button does:
    /// `show-modal` on a `dialog` that is also a popover is no menu
    var opensPopover: Bool { action == .show || action == .toggle }
    /// Whether it closes `target` while that shows modally or as a popover:
    /// HTML's dialog `close` closes a modal dialog, and `hide-popover` a popover.
    func closes(_ target: NodeView, modally: Bool) -> Bool {
        self.target === target && action == (modally && target.props["semanticTag"] == "dialog" ? .close : .hide)
    }

    /// What an `id` names in the session: a view, or an element that has
    /// none (a run of a paragraph), which no command acts on.
    enum Named { case view(NodeView), element }

    /// `nil` when `source` is not a button or invokes nothing. `element`
    /// finds what an `id` names, `nil` when nothing does.
    init?(of source: NodeView, element: (String) -> Named?) {
        guard source.isButton else { return nil }
        if let name = source.props["commandfor"], !name.isEmpty, let named = element(name) {
            guard case .view(let commanded) = named else { return nil }
            let popover = commanded.props["popover"] != nil, dialog = commanded.props["semanticTag"] == "dialog"
            switch source.props["command"].map(Self.asciiLowercased) {
            case "show-popover" where popover: action = .show
            case "hide-popover" where popover: action = .hide
            case "toggle-popover" where popover: action = .toggle
            case "show-modal" where dialog: action = .showModal
            case "close" where dialog: action = .close
            default: return nil
            }
            target = commanded
            return
        }
        guard let name = source.props["popovertarget"], !name.isEmpty, case .view(let popover)? = element(name),
              popover.props["popover"] != nil else { return nil }
        switch source.props["popovertargetaction"].map(Self.asciiLowercased) {
        case "show": action = .show
        case "hide": action = .hide
        default: action = .toggle
        }
        target = popover
    }

    fileprivate static func asciiLowercased(_ s: String) -> String {
        String(String.UnicodeScalarView(s.unicodeScalars.map { (65...90).contains($0.value) ? Unicode.Scalar($0.value + 32)! : $0 }))
    }
}

extension Presenter {
    /// What `source` invokes now, resolving names among this session's views
    /// (the lowest id among several with one `id`, as `aria-*` references do)
    /// and then its paragraphs' named runs.
    func invocation(of source: NodeView) -> Invocation? {
        Invocation(of: source, element: invocationElement)
    }

    #if !os(macOS)
    /// Contact can precede the press handler that mounts a named popover.
    /// Resolved targets retain their menu or overlay's existing feedback.
    func showsPopoverPress(of source: NodeView) -> Bool {
        if let invocation = invocation(of: source) {
            return invocation.opensPopover && !menus.pullsDown(invocation.target)
        }
        guard source.isButton else { return false }
        if let name = source.props["commandfor"], !name.isEmpty {
            if invocationElement(name) != nil { return false }
            if ["show-popover", "toggle-popover"].contains(source.props["command"].map(Invocation.asciiLowercased) ?? "") { return true }
        }
        guard let name = source.props["popovertarget"], !name.isEmpty, invocationElement(name) == nil else { return false }
        return source.props["popovertargetaction"].map(Invocation.asciiLowercased) != "hide"
    }
    #endif

    private func invocationElement(_ name: String) -> Invocation.Named? {
        if let view = chrome.named[name]?.min().flatMap({ views[$0] }) { return .view(view) }
        let run = inlineOwners.values.contains { location in
            guard let runs = views[location.owner]?.inlineText, runs.indices.contains(location.index) else { return false }
            return runs[location.index].props["id"] == name
        }
        return run ? .element : nil
    }
}

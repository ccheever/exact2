#if os(macOS)
import AppKit

/// HTML's top layer belongs to the session, not to NSApp or a second runner.
/// The actual subtree moves; fields, selection and app state remain the same.
final class DialogHost {
    private weak var presenter: Presenter?
    final class Entry {
        let dialog: NodeView
        weak var parent: NSView?
        weak var previous: NSView?
        let index: Int
        let selection: NSRange?
        var frame: NSRect
        let backdrop = DialogBackdrop(frame: .zero)
        init(_ dialog: NodeView, previous: NSView?) {
            self.dialog = dialog; self.previous = previous
            selection = ((previous as? NSTextField)?.currentEditor() as? NSTextView)?.selectedRange()
                ?? (previous as? NSTextView)?.selectedRange()
            parent = dialog.superview
            index = parent?.subviews.firstIndex(of: dialog) ?? 0
            frame = dialog.frame
        }
    }
    private var entries: [Entry] = []
    var active: NodeView? { entries.last?.dialog }
    /// LLP 1080.001 D3: an open dialog's backdrop; the dialogs it lifts.
    func inspectionOwns(_ view: NSView) -> Bool { entries.contains { $0.backdrop === view } }
    func projects(_ node: NodeView) -> Bool { entries.contains { $0.dialog === node } }
    var presented: [NodeView] { entries.map(\.dialog) }
    init(_ presenter: Presenter) { self.presenter = presenter }

    func owns(_ node: NodeView) -> Bool { entries.contains { $0.dialog === node } }
    func blocks(_ node: NSView) -> Bool {
        guard let active else { return false }
        return presenter?.menus.contains(active, node) != true
    }
    private func live(_ node: NodeView) -> Bool { presenter?.views[node.id] === node }
    private func focusOwner(_ window: NSWindow) -> NSView? {
        guard let view = window.firstResponder as? NSView else { return nil }
        if let editor = view as? NSTextView, editor.isFieldEditor {
            return editor.delegate as? NSView ?? editor
        }
        return view
    }
    /// Capture the command before the press's app code can replace its source.
    func command(_ source: NodeView, fromNativeMenu: Bool = false) -> (() -> Void)? {
        guard !source.disabled, !source.inert, (fromNativeMenu || !source.isHiddenOrHasHiddenAncestor || presenter?.toolbar.contains(source) == true),
              let name = source.props["commandfor"], let command = source.props["command"],
              let target = presenter?.carrying("tag:dialog").first(where: { $0.props["id"] == name }) else { return nil }
        return { [weak self, weak source, weak target] in
            guard let self, let source, let target, self.live(source), self.live(target),
                  source.props["commandfor"] == name, source.props["command"] == command else { return }
            if command == "show-modal" { self.show(target) }
            else if command == "close" { self.close(target) }
        }
    }
    func show(_ dialog: NodeView) {
        guard let presenter, !owns(dialog), live(dialog), let window = presenter.viewport.window,
              let parent = dialog.superview, parent.isDescendant(of: presenter.root) || parent === presenter.root
                || entries.contains(where: { parent.isDescendant(of: $0.dialog) || parent === $0.dialog }) else { return }
        let entry = Entry(dialog, previous: focusOwner(window))
        presenter.menus.reset()
        entries.append(entry)
        entry.backdrop.host = self
        entry.backdrop.autoresizingMask = [.width, .height]
        presenter.viewport.addSubview(entry.backdrop)
        entry.backdrop.addSubview(dialog)
        dialog.isHidden = false
        layout()
        presenter.mouseSwipe.cancel(); presenter.mouseHeightDrag.cancel()
        presenter.mouseTransformDrag.cancel(); presenter.mouseReorder.cancel()
        presenter.mouseLayoutPan.abandon(); presenter.selection.clear()
        presenter.session?.canvases.cancelMovedControls()
        refreshInputGates()
        presenter.syncKeyViewLoop()
        enterFocus()
        presenter.syncAccessibility()
        presenter.shortcuts.sync()
    }
    func close(_ dialog: NodeView, restoreFocus: Bool = true) {
        guard let i = entries.firstIndex(where: { $0.dialog === dialog }) else { return }
        for popover in presenter?.menus.presented ?? [] where presenter?.menus.contains(dialog, popover) == true {
            presenter?.menus.close(popover, restoreFocus: false)
        }
        let window = presenter?.viewport.window
        let hadFocus = window.map { ownsFocus($0) } ?? false
        let entry = entries.remove(at: i)
        let wasTop = i == entries.count
        dialog.isHidden = true
        dialog.removeFromSuperview()
        entry.backdrop.removeFromSuperview()
        if live(dialog), let parent = entry.parent {
            let siblings = parent.subviews
            parent.addSubview(dialog, positioned: .below, relativeTo: entry.index < siblings.count ? siblings[entry.index] : nil)
            dialog.frame = entry.frame
        }
        refreshInputGates()
        presenter?.syncKeyViewLoop()
        if wasTop, restoreFocus, hadFocus, let window {
            if let previous = entry.previous, previous.window === window,
               !previous.isHiddenOrHasHiddenAncestor, !blocks(previous),
               !((previous as? NodeView)?.inert ?? false) {
                window.makeFirstResponder(previous)
                if let range = entry.selection, let editor = window.firstResponder as? NSTextView { editor.setSelectedRange(range) }
            } else if active != nil { enterFocus() }
            else { window.makeFirstResponder(presenter?.viewport) }
        }
        presenter?.syncAccessibility()
        presenter?.shortcuts.sync()
    }
    func reset() {
        while let dialog = active { close(dialog, restoreFocus: false) }
    }
    /// A children op still addresses the authored parent, even while a dialog
    /// is in the top layer. Removing that edge retires its presentation.
    func children(_ parent: NSView, _ wanted: [NodeView]) {
        for entry in entries where (entry.parent === parent) != wanted.contains(where: { $0 === entry.dialog }) {
            close(entry.dialog)
        }
    }
    func frame(_ node: NodeView, _ frame: NSRect) -> Bool {
        guard let entry = entries.first(where: { $0.dialog === node }) else { return false }
        entry.frame = frame
        layout()
        return true
    }
    func sync() {
        guard let presenter else { return }
        for entry in entries {
            let parent = entry.parent
            let connected = parent === presenter.root || parent?.isDescendant(of: presenter.root) == true
                || entries.contains { other in other !== entry && (parent === other.dialog || parent?.isDescendant(of: other.dialog) == true) }
            if !live(entry.dialog) || entry.dialog.props["semanticTag"] != "dialog" || !connected || presenter.viewport.window == nil {
                close(entry.dialog)
            }
        }
        var changed = false
        for dialog in presenter.carrying("tag:dialog") where dialog.props["popover"] == nil {
            let hidden = !owns(dialog) || dialog.style["display"]?.string == "none"
            if dialog.isHidden != hidden { dialog.isHidden = hidden; changed = true }
        }
        if changed { refreshInputGates() }
        layout()
        if let window = presenter.viewport.window, active != nil,
           ownsFocus(window), focusOwner(window).map({ blocks($0) }) ?? true { enterFocus() }
    }
    private func refreshInputGates() {
        guard let presenter else { return }
        presenter.navigation.refreshInputGates(Array(presenter.views.values))
        presenter.topLayerChanged()
    }
    func layout() {
        guard let presenter else { return }
        for entry in entries {
            entry.backdrop.frame = presenter.viewport.bounds
            var frame = entry.frame
            // Without authored insets the HTML modal is centred in its viewport.
            let style = entry.dialog.style
            if let left = length(style["left"], entry.backdrop.bounds.width) { frame.origin.x = left }
            else if let right = length(style["right"], entry.backdrop.bounds.width) { frame.origin.x = entry.backdrop.bounds.width - right - frame.width }
            else {
                frame.origin.x = max(0, (entry.backdrop.bounds.width - frame.width) / 2)
            }
            if let top = length(style["top"], entry.backdrop.bounds.height) { frame.origin.y = top }
            else if let bottom = length(style["bottom"], entry.backdrop.bounds.height) { frame.origin.y = entry.backdrop.bounds.height - bottom - frame.height }
            else {
                frame.origin.y = max(0, (entry.backdrop.bounds.height - frame.height) / 2)
            }
            entry.dialog.frame = frame
        }
    }
    private func length(_ value: BatchValue?, _ basis: CGFloat) -> CGFloat? {
        if let n = value?.number { return CGFloat(n) }
        if case .object(let parts) = value {
            return CGFloat(parts["pct"]?.number ?? 0) * basis / 100 + CGFloat(parts["px"]?.number ?? 0)
        }
        return nil
    }
    private func candidates(_ dialog: NodeView) -> [NSView] {
        guard let presenter else { return [] }
        var nodes: [NodeView] = []
        func walk(_ node: NodeView) {
            guard !node.inert, !node.disabled, !node.isHidden else { return }
            if Presenter.tabbable(node) { nodes.append(node) }
            for case let child as NodeView in node.container.subviews { walk(child) }
            for popover in presenter.menus.following(node) { walk(popover) }
        }
        walk(dialog)
        return nodes.enumerated().sorted { a, b in
            let ai = a.element.tabOrder, bi = b.element.tabOrder
            let ap = ai > 0 ? ai : Int.max, bp = bi > 0 ? bi : Int.max
            return ap == bp ? a.offset < b.offset : ap < bp
        }.map { presenter.keyView(of: $0.element) }
    }
    func enterFocus() {
        guard let presenter, let dialog = active, let window = dialog.window else { return }
        let candidates = candidates(dialog)
        let autofocus = presenter.carrying("autofocus").first {
            ($0 === dialog || $0.isDescendant(of: dialog)) && $0.props["autofocus"] == "true" && !$0.inert && !$0.disabled && !$0.isHiddenOrHasHiddenAncestor
        }
        let focus = autofocus.map { presenter.keyView(of: $0) } ?? candidates.first ?? dialog
        window.makeFirstResponder(focus)
    }
    private func ownsFocus(_ window: NSWindow) -> Bool {
        guard let owner = focusOwner(window), let viewport = presenter?.viewport else { return true }
        return owner === viewport || owner.isDescendant(of: viewport) || owner === presenter?.session?.view
    }
    /// Shared by real key delivery and the agent's direct NSWindow delivery.
    func key(_ event: NSEvent) -> Bool {
        guard let dialog = active, event.window === dialog.window else { return false }
        if let window = dialog.window, !ownsFocus(window) { return false }
        // Composition owns Escape and Tab until the input method commits it.
        if (dialog.window?.firstResponder as? NSTextInputClient)?.hasMarkedText() == true { return false }
        if event.keyCode == 53 {
            if event.type == .keyDown, !event.isARepeat, dialog.props["closedby"] != "none" { close(dialog) }
            return true
        }
        if event.type == .keyDown, event.keyCode == 48 {
            let choices = candidates(dialog)
            guard let window = dialog.window, !choices.isEmpty else { enterFocus(); return true }
            let current = focusOwner(window).flatMap { owner -> Int? in
                var ancestor: NSView? = owner
                while let view = ancestor {
                    if let index = choices.firstIndex(where: { $0 === view
                        || ($0 as? NSTextField)?.currentEditor() === window.firstResponder }) { return index }
                    ancestor = view.superview
                }
                return nil
            }
            let backwards = event.modifierFlags.contains(.shift)
            let index = current.map { ($0 + (backwards ? choices.count - 1 : 1)) % choices.count }
                ?? (backwards ? choices.count - 1 : 0)
            window.makeFirstResponder(choices[index])
            return true
        }
        if let window = dialog.window, focusOwner(window).map({ blocks($0) }) ?? true { enterFocus() }
        return false
    }
    func outside() {
        if let dialog = active, dialog.props["closedby"] == "any" { close(dialog) }
    }
    var observation: [String: Any]? {
        active.map { ["kind": "dialog", "dialog": Int($0.id), "closedby": $0.props["closedby"] ?? "closerequest", "phase": "open"] }
    }
}

final class DialogBackdrop: NSView {
    weak var host: DialogHost?
    private var downOutside = false
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) { NSColor.black.withAlphaComponent(0.18).setFill(); dirtyRect.fill() }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    private func outside(_ event: NSEvent) -> Bool {
        guard let parent = superview else { return false }
        return hitTest(parent.convert(event.locationInWindow, from: nil)) === self
    }
    override func mouseDown(with event: NSEvent) { downOutside = outside(event) }
    override func mouseUp(with event: NSEvent) {
        defer { downOutside = false }
        if downOutside, outside(event) { host?.outside() }
    }
    override func scrollWheel(with event: NSEvent) {}
}
#endif

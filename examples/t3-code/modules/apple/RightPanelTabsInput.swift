import AppKit

/// The device tab's rename editor keys (RightPanelTabs.tsx: its input's keydown stops propagation, and
/// Escape restores the title and blurs). Contract cannot do either: an `aria-keyshortcuts` button
/// hears its chord before any field's `key` handler and takes it, and the window holds dozens of
/// Escape buttons. So Escape presses the tab's cancel button, and a ⌘ or ⌃ chord goes to the field
/// editor alone, never to a window or tab shortcut. The middle click that closes a tab is the tab's
/// own `pointerdown`/`pointerup` (panel.contract PanelTabChip).
final class RightPanelTabsInput {
    private final class Weak { weak var element: ExactElement?; init(_ element: ExactElement) { self.element = element } }
    private var elements: [String: Weak] = [:]
    private var monitor: Any?
    func install(_ element: ExactElement) {
        // The rename editor is the anchor `tab-editor:<id>` (panel.contract PanelTabNameField).
        guard element.hatch == .t3Anchor, let name = element.data[.anchor],
              ["tab-cancel:", "tab-focus:", "tab-editor:"].contains(where: name.hasPrefix) else { return }
        elements[name] = Weak(element)
        if monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { [weak self] event in
                guard let self else { return event }
                return self.handle(event)
            }
        }
    }
    func remove(_ element: ExactElement) {
        if element.id.hasPrefix("tab-name-"), element.textField?.currentEditor() != nil {
            let id = String(element.id.dropFirst("tab-name-".count))
            DispatchQueue.main.async { [weak self] in self?.elements["tab-focus:\(id)"]?.element?.focus() }
        }
        elements = elements.filter { $0.value.element != nil && $0.value.element !== element }
    }
    func destroy() { if let monitor { NSEvent.removeMonitor(monitor) }; monitor = nil; elements.removeAll() }
    func handle(_ event: NSEvent) -> NSEvent? {
        guard let window = event.window, event.type == .keyDown else { return event }
        for (name, item) in elements where name.hasPrefix("tab-editor:") {
            guard let field = item.element?.textField, let editor = field.currentEditor() as? NSTextView,
                  window.firstResponder === editor, !editor.hasMarkedText() else { continue }
            if event.keyCode == 53 {
                let id = String(name.dropFirst("tab-editor:".count))
                elements["tab-cancel:\(id)"]?.element?.click()
                return nil
            }
            if !event.modifierFlags.intersection([.command, .control]).isEmpty {
                // Text-editing shortcuts stay in the editor; an unhandled chord
                // must not become a window/tab shortcut while renaming.
                _ = editor.performKeyEquivalent(with: event)
                return nil
            }
            return event
        }
        return event
    }
}

import AppKit

/// Extra mouse buttons and double-clicks are AppKit facts. Dispatch through the
/// existing Contract buttons so these paths share guards, persistence and cleanup.
final class RightPanelTabsInput {
    private final class Weak { weak var element: ExactElement?; init(_ element: ExactElement) { self.element = element } }
    private var elements: [String: Weak] = [:]
    private var monitor: Any?
    func install(_ element: ExactElement) {
        if element.hook == .t3SelectOnOpen, element.id.hasPrefix("tab-name-") {
            elements["tab-editor:" + String(element.id.dropFirst("tab-name-".count))] = Weak(element)
        } else {
            guard element.hook == .t3Anchor, let name = element.data[.anchor],
                  ["r12-tab:", "tab-close:", "tab-rename:", "tab-cancel:", "tab-menu:", "tab-focus:"].contains(where: name.hasPrefix) else { return }
            elements[name] = Weak(element)
        }
        if monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.otherMouseDown, .leftMouseDown, .keyDown]) { [weak self] event in
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
        guard let window = event.window else { return event }
        if event.type == .keyDown {
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
            if event.keyCode == 109, event.modifierFlags.contains(.shift), let focused = window.firstResponder as? NSView {
                for (name, item) in elements where name.hasPrefix("r12-tab:") {
                    guard let view = item.element?.view, focused === view || focused.isDescendant(of: view) else { continue }
                    elements["tab-menu:" + String(name.dropFirst("r12-tab:".count))]?.element?.click()
                    return nil
                }
            }
            return event
        }
        let action: String
        if event.type == .otherMouseDown && event.buttonNumber == 2 { action = "tab-close:" }
        else if event.type == .leftMouseDown && event.clickCount == 2 { action = "tab-rename:" }
        else { return event }
        for (name, item) in elements where name.hasPrefix("r12-tab:") {
            guard let element = item.element, element.isLive, let view = element.view, view.window === window,
                  !view.isHiddenOrHasHiddenAncestor, view.visibleRect.contains(view.convert(event.locationInWindow, from: nil)) else { continue }
            // Double-clicking the close glyph must not start rename.
            let id = String(name.dropFirst("r12-tab:".count))
            if action == "tab-rename:", let close = elements["tab-close:\(id)"]?.element?.view,
               close.bounds.contains(close.convert(event.locationInWindow, from: nil)) { return event }
            elements[action + id]?.element?.click()
            return nil
        }
        return event
    }
}

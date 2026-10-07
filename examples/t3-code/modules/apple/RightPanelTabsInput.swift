import AppKit

/// The middle button and the rename editor's keys are AppKit facts. Dispatch through the
/// existing Contract buttons so these paths share guards, persistence and cleanup. A
/// double-click on the title is the tab button's own Contract `dblclick` (agent-drivable). The
/// agent's `tap panel-tab-<id> auxclick` reaches the monitor as a hand's middle click does
/// (NSApp.sendEvent, button 2; exact2 #186).
final class RightPanelTabsInput {
    private final class Weak { weak var element: ExactElement?; init(_ element: ExactElement) { self.element = element } }
    private var elements: [String: Weak] = [:]
    private var monitor: Any?
    func install(_ element: ExactElement) {
        if element.hatch == .t3SelectOnOpen, element.id.hasPrefix("tab-name-") {
            elements["tab-editor:" + String(element.id.dropFirst("tab-name-".count))] = Weak(element)
        } else {
            guard element.hatch == .t3Anchor, let name = element.data[.anchor],
                  ["r12-tab:", "tab-close:", "tab-cancel:", "tab-focus:"].contains(where: name.hasPrefix) else { return }
            elements[name] = Weak(element)
        }
        if monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.otherMouseDown, .keyDown]) { [weak self] event in
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
            return event
        }
        guard event.type == .otherMouseDown && event.buttonNumber == 2 else { return event }
        for (name, item) in elements where name.hasPrefix("r12-tab:") {
            guard let element = item.element, element.isLive, let view = element.view, view.window === window,
                  !view.isHiddenOrHasHiddenAncestor,
                  // visibleRect alone is not clipped to the view's bounds (an unclipped NSView reports
                  // its ancestors' visible area), so every tab matched and the first in the
                  // dictionary closed: the agent's middle click (exact2 #186) showed it.
                  view.bounds.intersection(view.visibleRect).contains(view.convert(event.locationInWindow, from: nil)) else { continue }
            elements["tab-close:" + String(name.dropFirst("r12-tab:".count))]?.element?.click()
            return nil
        }
        return event
    }
}

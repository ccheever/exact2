#if os(macOS)
import AppKit
import ExactKit

/// Only the canvas hit takes raw input. A held pointer stays with that canvas.
final class CanvasInputHost {
    weak var view: NodeView?
    private var tracking: NSTrackingArea?
    private var inactive: NSObjectProtocol?
    private var resigned: NSObjectProtocol?
    private var keys: Set<String> = []
    private var buttons = 0
    private var modifiers: Set<String> = []
    private var last: NSPoint?
    /// The mouse is captured for mouse look (`data-pointer-lock="true"`): the
    /// cursor is hidden and held still, and moves carry the device's deltas.
    private(set) var locked = false

    init(view: NodeView) {
        self.view = view
        updateTracking()
        inactive = NotificationCenter.default.addObserver(forName: NSApplication.willResignActiveNotification, object: nil, queue: .main) { [weak self] _ in self?.blur() }
        // Another window taking the keyboard while the app stays active must not
        // leave the cursor hidden and held.
        resigned = NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification, object: nil, queue: .main) { [weak self] note in
            if let self, note.object as? NSWindow === self.view?.window { self.unlock() }
        }
        focusIfUnheld()
        DispatchQueue.main.async { [weak self] in self?.focusIfUnheld() }
    }
    private func focusIfUnheld() {
        guard let v = view, v.canvases?.wantsInput(v.id) == true, let window = v.window else { return }
        let current = window.firstResponder
        if current == nil || current === window || current === window.contentView || current === v.presenter?.viewport || current === v.presenter?.session?.view { _ = v.focusCanvas() }
    }
    deinit {
        unlock()
        if let inactive { NotificationCenter.default.removeObserver(inactive) }
        if let resigned { NotificationCenter.default.removeObserver(resigned) }
        if let tracking { view?.removeTrackingArea(tracking) }
    }
    func updateTracking() {
        guard let view else { return }
        if let tracking { view.removeTrackingArea(tracking) }
        let area = NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect], owner: view, userInfo: nil)
        view.addTrackingArea(area)
        tracking = area
    }
    /// The web's `requestPointerLock` on AppKit: the cursor leaves the
    /// mouse's control until Escape, blur or the canvas goes away.
    private func lock() {
        guard !locked else { return }
        locked = CGAssociateMouseAndMouseCursorPosition(0) == .success
        if locked { NSCursor.hide() }
    }
    func unlock() {
        guard locked else { return }
        locked = false
        CGAssociateMouseAndMouseCursorPosition(1)
        NSCursor.unhide()
    }
    private var lockable: Bool {
        guard let data = view?.props["dataset"]?.data(using: .utf8),
              let words = try? JSONSerialization.jsonObject(with: data) as? [String: String] else { return false }
        return words["pointer-lock"] == "true"
    }
    func blur() {
        guard let view else { return }
        unlock()
        buttons = 0; modifiers.removeAll(); keys.removeAll(); last = nil
        if let c=view.canvases as? CanvasesHost, let e=c.entries[view.id] {c.cancelControls(e)}
        view.canvases?.input(view, ["t": "blur"])
    }
    func key(_ event: NSEvent, down: Bool, source: NodeView) -> Bool {
        guard let view else { return false }
        let code = KeyCodes.mac[Int(event.keyCode)] ?? "Unidentified"
        if down && code == "Escape" { unlock() }
        if (source.isSurfaceControl || !down) && ["Space", "Enter", "NumpadEnter"].contains(code) {
            if down && event.isARepeat { return true }
            if source.controlKey(code, down: down, timestamp: event.timestamp) { return true }
        }
        if down {
            guard view.window?.firstResponder === source,
                  source.forwardsCanvasKey(code, command: !event.modifierFlags.intersection([.command, .control]).isEmpty) else { return false }
            keys.insert(code)
        } else if keys.remove(code) == nil { return false }
        // A named key's characters are its function character, so the caret
        // moves; the surface still hears the name (`End`), as the web does.
        let key = KeyCodes.named(code) ? KeyCodes.key(code) : (event.characters.flatMap { $0.isEmpty ? nil : $0 } ?? KeyCodes.key(code))
        view.canvases?.input(view, ["t": "key", "code": code, "key": key, "down": down, "repeat": event.isARepeat], timestamp: event.timestamp)
        return !event.modifierFlags.contains(.command)
    }
    func flags(_ event: NSEvent) -> Bool {
        guard let view, view.window?.firstResponder === view, let code = KeyCodes.mac[Int(event.keyCode)] else { return false }
        // NX_DEVICE* masks distinguish releasing one side while the other is held.
        guard let (side, pair, flag) = KeyCodes.sides[code] else { return false }
        let raw = event.modifierFlags.rawValue
        let down = raw & pair != 0 ? raw & side != 0 : event.modifierFlags.contains(flag) && !modifiers.contains(code)
        if down { modifiers.insert(code) } else { modifiers.remove(code) }
        view.canvases?.input(view, ["t": "key", "code": code, "key": KeyCodes.key(code), "down": down, "repeat": false], timestamp: event.timestamp)
        return !event.modifierFlags.contains(.command)
    }
    private func windowPoint(_ event: NSEvent, _ view: NodeView) -> NSPoint {
        // A driver-created CG wheel has screen coordinates and no event.window.
        if event.window == nil, let cg = event.cgEvent, let window = view.window {
            return window.convertPoint(fromScreen: NSPoint(x: cg.location.x, y: (NSScreen.screens.first?.frame.height ?? 0) - cg.location.y))
        }
        return event.locationInWindow
    }
    private func fallsThrough(_ event: NSEvent, _ view: NodeView) -> Bool {
        guard let root = view.window?.contentView else { return false }
        return root.hitTest(root.superview?.convert(windowPoint(event, view), from: nil) ?? windowPoint(event, view)) === view
    }
    func pointer(_ event: NSEvent, phase: String) -> Bool {
        guard let view, !view.disabled, !view.inert else { return false }
        // The event's type names left and right; an event made with
        // NSEvent.mouseEvent (the agent's) carries buttonNumber 0 for both.
        let bit = [.leftMouseDown, .leftMouseUp, .leftMouseDragged].contains(event.type) ? 1
            : [.rightMouseDown, .rightMouseUp, .rightMouseDragged].contains(event.type) ? 2
            // Other: the middle button (2), or 0 when made by NSEvent.mouseEvent.
            : [.otherMouseDown, .otherMouseUp, .otherMouseDragged].contains(event.type) && event.buttonNumber <= 2 ? 4
            : 1 << min(event.buttonNumber, 30)
        var phase = phase
        if phase == "down" {
            guard locked || fallsThrough(event, view) else { return false }
            _ = view.focusSurfacePointer()
            // A second button joins the held contact as a move, as the web's chorded buttons do.
            if buttons != 0 { phase = "move" }
            buttons |= bit
            if lockable { lock() }
        } else if phase == "up" {
            guard buttons & bit != 0 else { return false }
            buttons &= ~bit
            if buttons != 0 { phase = "move" }
        } else if buttons == 0 && !locked && !fallsThrough(event, view) {
            // Off the canvas the pointer still moves: the first move back in is
            // its own motion, not the distance travelled outside.
            last = view.local(windowPoint(event, view)); return false
        }
        let point = view.local(windowPoint(event, view))
        // Locked, the cursor stays put and the motion is the device's (unbounded
        // by the window or screen edge); otherwise it is the position's change.
        let moved = [.mouseMoved, .leftMouseDragged, .rightMouseDragged, .otherMouseDragged].contains(event.type)
        let (dx, dy) = !moved ? (0, 0) : locked ? (event.deltaX, event.deltaY) : last.map { (point.x - $0.x, point.y - $0.y) } ?? (0, 0)
        last = point
        view.canvases?.input(view, ["t": "pointer", "phase": phase, "id": 1, "x": point.x, "y": point.y, "dx": dx, "dy": dy, "kind": "mouse", "buttons": buttons], timestamp: event.timestamp)
        return true
    }
    func wheel(_ event: NSEvent) -> Bool {
        guard let view, fallsThrough(event, view), !view.disabled, !view.inert else { return false }
        let point = view.local(windowPoint(event, view))
        view.canvases?.input(view, ["t": "wheel", "dx": -event.scrollingDeltaX, "dy": -event.scrollingDeltaY, "x": point.x, "y": point.y], timestamp: event.timestamp)
        return true
    }
}

#endif

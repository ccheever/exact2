#if os(macOS)
import AppKit

/// Only the canvas hit takes raw input. A held pointer stays with that canvas.
final class CanvasInput {
    weak var view: NodeView?
    private var tracking: NSTrackingArea?
    private var inactive: NSObjectProtocol?
    private var keys: Set<String> = []
    private var buttons = 0
    private var modifiers: Set<String> = []

    init(view: NodeView) {
        self.view = view
        updateTracking()
        inactive = NotificationCenter.default.addObserver(forName: NSApplication.willResignActiveNotification, object: nil, queue: .main) { [weak self] _ in self?.blur() }
        focusIfUnheld()
        DispatchQueue.main.async { [weak self] in self?.focusIfUnheld() }
    }
    private func focusIfUnheld() {
        guard let v = view, v.canvases?.wantsInput(v.id) == true, let window = v.window else { return }
        let current = window.firstResponder
        if current == nil || current === window || current === window.contentView || current === v.presenter?.viewport || current === v.presenter?.session?.view { _ = v.focusCanvas() }
    }
    deinit {
        if let inactive { NotificationCenter.default.removeObserver(inactive) }
        if let tracking { view?.removeTrackingArea(tracking) }
    }
    func updateTracking() {
        guard let view else { return }
        if let tracking { view.removeTrackingArea(tracking) }
        let area = NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect], owner: view, userInfo: nil)
        view.addTrackingArea(area)
        tracking = area
    }
    func blur() {
        guard let view else { return }
        buttons = 0; modifiers.removeAll(); keys.removeAll()
        view.canvases?.input(view, ["t": "blur"])
    }
    func key(_ event: NSEvent, down: Bool, source: NodeView) -> Bool {
        guard let view else { return false }
        let code = KeyCodes.mac[Int(event.keyCode)] ?? "Unidentified"
        if down {
            guard view.window?.firstResponder === source,
                  source.forwardsCanvasKey(code, command: !event.modifierFlags.intersection([.command, .control]).isEmpty) else { return false }
            keys.insert(code)
        } else if keys.remove(code) == nil { return false }
        let key = event.characters.flatMap { $0.isEmpty ? nil : $0 } ?? KeyCodes.key(code)
        view.canvases?.input(view, ["t": "key", "code": code, "key": key, "down": down, "repeat": event.isARepeat], timestamp: event.timestamp)
        return !event.modifierFlags.contains(.command)
    }
    func flags(_ event: NSEvent) -> Bool {
        guard let view, view.window?.firstResponder === view, let code = KeyCodes.mac[Int(event.keyCode)] else { return false }
        // NX_DEVICE* masks distinguish releasing one side while the other is held.
        let masks: [String: (UInt, UInt, NSEvent.ModifierFlags)] = [
            "ShiftLeft": (0x2, 0x6, .shift), "ShiftRight": (0x4, 0x6, .shift),
            "ControlLeft": (0x1, 0x2001, .control), "ControlRight": (0x2000, 0x2001, .control),
            "AltLeft": (0x20, 0x60, .option), "AltRight": (0x40, 0x60, .option),
            "MetaLeft": (0x8, 0x18, .command), "MetaRight": (0x10, 0x18, .command),
        ]
        guard let (side, pair, flag) = masks[code] else { return false }
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
        let bit = event.buttonNumber == 0 ? 1 : event.buttonNumber == 1 ? 2 : event.buttonNumber == 2 ? 4 : 1 << min(event.buttonNumber, 30)
        if phase == "down" {
            guard fallsThrough(event, view) else { return false }
            _ = view.focusCanvas()
            buttons |= bit
        } else if phase == "up" {
            guard buttons & bit != 0 else { return false }
            buttons &= ~bit
        } else if buttons == 0 && !fallsThrough(event, view) { return false }
        let point = view.local(windowPoint(event, view))
        view.canvases?.input(view, ["t": "pointer", "phase": phase, "id": 1, "x": point.x, "y": point.y, "kind": "mouse", "buttons": buttons], timestamp: event.timestamp)
        return true
    }
    func wheel(_ event: NSEvent) -> Bool {
        guard let view, fallsThrough(event, view), !view.disabled, !view.inert else { return false }
        let point = view.local(windowPoint(event, view))
        view.canvases?.input(view, ["t": "wheel", "dx": -event.scrollingDeltaX, "dy": -event.scrollingDeltaY, "x": point.x, "y": point.y], timestamp: event.timestamp)
        return true
    }
}

extension NodeView {
    var canvasScale: CGFloat { metal?.layer?.contentsScale ?? window?.backingScaleFactor ?? 1 }
    func focusCanvas() -> Bool {
        guard canvases?.wantsInput(id) == true, acceptsFirstResponder, let window else { return false }
        if window.firstResponder !== self { window.makeFirstResponder(self) }
        return window.firstResponder === self
    }
}
#endif

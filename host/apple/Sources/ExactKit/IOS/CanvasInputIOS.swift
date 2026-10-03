#if os(iOS)
import GameController
import UIKit

/// Whether a canvas asks for iPadOS pointer lock (`data-pointer-lock="true"`,
/// after a press on it): an app's view controller answers `prefersPointerLocked`
/// with this, as ExactIOS's does.
public enum ExactPointerLock {
    public static var preferred: Bool { CanvasInput.lockOwner != nil }
}

final class CanvasInput {
    weak var view: NodeView?
    private var inactive: NSObjectProtocol?
    private var touches: [ObjectIdentifier: Int] = [:]
    private var keys: Set<String> = []
    private var nextTouch = 1
    private let multiple: Bool

    init(view: NodeView) {
        self.view = view
        multiple = view.isMultipleTouchEnabled
        view.isMultipleTouchEnabled = true
        inactive = NotificationCenter.default.addObserver(forName: UIApplication.willResignActiveNotification, object: nil, queue: .main) { [weak self] _ in self?.blur() }
        focusIfUnheld()
        DispatchQueue.main.async { [weak self] in self?.focusIfUnheld() }
    }
    private func focusIfUnheld() {
        guard let v = view, let window = v.window, !CanvasInput.hasFocus(window) else { return }
        _ = v.focusCanvas()
    }
    deinit {
        unlock()
        if let inactive { NotificationCenter.default.removeObserver(inactive) }
        view?.isMultipleTouchEnabled = multiple
    }
    /// iPadOS pointer lock for a canvas marked `data-pointer-lock="true"`: the
    /// app's controller answers `prefersPointerLocked` from this, and while the
    /// scene is locked a connected mouse's raw motion and its three buttons
    /// (GameController's `GCMouse`) reach the canvas as the web's locked pointer.
    /// One canvas owns the request at a time.
    nonisolated(unsafe) static private(set) weak var lockOwner: CanvasInput?
    private var locking = false, mouseButtons = 0
    private var lockable: Bool {
        guard let data = view?.props["dataset"]?.data(using: .utf8),
              let words = try? JSONSerialization.jsonObject(with: data) as? [String: String] else { return false }
        return words["pointer-lock"] == "true"
    }
    private var sceneLocked: Bool { view?.window?.windowScene?.pointerLockState?.isLocked == true }
    private func lock() {
        guard !locking, lockable, let input = GCMouse.current?.mouseInput else { return }
        CanvasInput.lockOwner?.unlock()
        locking = true; CanvasInput.lockOwner = self
        view?.window?.rootViewController?.setNeedsUpdateOfPrefersPointerLocked()
        input.mouseMovedHandler = { [weak self] _, dx, dy in self?.locked(dx: CGFloat(dx), dy: CGFloat(-dy), bit: 0, down: false) }
        for (bit, button) in [(1, input.leftButton), (2, input.rightButton), (4, input.middleButton)] {
            button?.pressedChangedHandler = { [weak self] _, _, down in self?.locked(dx: 0, dy: 0, bit: bit, down: down) }
        }
    }
    func unlock() {
        guard locking else { return }
        locking = false; mouseButtons = 0
        if CanvasInput.lockOwner === self { CanvasInput.lockOwner = nil }
        if let input = GCMouse.current?.mouseInput {
            input.mouseMovedHandler = nil
            for button in [input.leftButton, input.rightButton, input.middleButton] { button?.pressedChangedHandler = nil }
        }
        view?.window?.rootViewController?.setNeedsUpdateOfPrefersPointerLocked()
    }
    /// The press that asked for the lock went out as a touch whose up the
    /// locked scene never delivers: end every live touch before GCMouse speaks.
    private func cancelTouches() {
        guard let view else { return }
        for (_, id) in touches.sorted(by: { $0.value < $1.value }) {
            view.canvases?.input(view, ["t": "pointer", "phase": "cancel", "id": id, "x": 0, "y": 0, "dx": 0, "dy": 0, "kind": "mouse", "buttons": 0])
        }
        touches.removeAll()
    }
    private func locked(dx: CGFloat, dy: CGFloat, bit: Int, down: Bool) {
        guard let view, sceneLocked else { return }
        cancelTouches()
        let before = mouseButtons
        if down { mouseButtons |= bit } else { mouseButtons &= ~bit }
        let phase = bit == 0 || (before != 0 && mouseButtons != 0) ? "move" : down ? "down" : "up"
        let p = CGPoint(x: view.bounds.midX, y: view.bounds.midY)
        view.canvases?.input(view, ["t": "pointer", "phase": phase, "id": 1, "x": p.x, "y": p.y, "dx": dx, "dy": dy, "kind": "mouse", "buttons": mouseButtons])
    }
    private static func hasFocus(_ view: UIView) -> Bool { view.isFirstResponder || view.subviews.contains(where: hasFocus) }
    func blur() {
        unlock()
        touches.removeAll(); keys.removeAll()
        if let view {
            if let c=view.canvases, let e=c.entries[view.id] {c.cancelControls(e)}
            view.canvases?.input(view, ["t": "blur"])
        }
    }
    func touches(_ values: Set<UITouch>, phase: String, source: NodeView, event: UIEvent? = nil) -> Bool {
        guard let view, phase != "down" || (!view.disabled && !view.inert) else { return false }
        var sent = false
        for touch in values where touch.view === source || source.isSurfaceControl {
            let token = ObjectIdentifier(touch)
            if phase == "down" {
                _ = (source.isSurfaceControl ? source : view).focusSurfacePointer()
                touches[token] = nextTouch
                nextTouch += 1
            }
            guard let id = touches[token] else { continue }
            let p = source.local(touch.location(in: nil))
            if source.isSurfaceControl || source.ownsSurfaceControl {
                let ok = source.control(phase, id: id, point: p, timestamp: touch.timestamp)
                if phase == "up" || phase == "cancel" { touches.removeValue(forKey: token) }
                sent = ok || sent; continue
            }
            if phase == "down" { lock() }
            if sceneLocked { cancelTouches(); continue } // the locked mouse speaks through GCMouse
            let from = phase == "down" ? p : source.local(touch.previousLocation(in: nil))
            // An iPad's mouse or trackpad (an indirect pointer) reports its buttons.
            let mouse = touch.type == .indirectPointer, mask = event?.buttonMask ?? []
            let buttons = phase == "up" || phase == "cancel" ? 0 : !mouse ? 1
                : (mask.contains(.primary) ? 1 : 0) | (mask.contains(.secondary) ? 2 : 0) | (mask.contains(.button(3)) ? 4 : 0)
            view.canvases?.input(view, ["t": "pointer", "phase": phase, "id": id, "x": p.x, "y": p.y, "dx": p.x - from.x, "dy": p.y - from.y, "kind": mouse ? "mouse" : "touch", "buttons": buttons], timestamp: touch.timestamp)
            if phase == "up" || phase == "cancel" { touches.removeValue(forKey: token) }
            sent = true
        }
        return sent
    }
    func presses(_ presses: Set<UIPress>, down: Bool, source: NodeView) -> Bool {
        guard let view else { return false }
        var handled = false
        for press in presses {
            guard let key = press.key else { continue }
            let code = KeyCodes.hid(key.keyCode.rawValue)
            if down && code == "Escape" { unlock() } // the web's way out of a lock
            if (source.isSurfaceControl || !down) && ["Space", "Enter", "NumpadEnter"].contains(code) {
                if down && keys.contains(code) { handled = true; continue }
                if down { keys.insert(code) } else { keys.remove(code) }
                if source.controlKey(code, down: down, timestamp: press.timestamp) { handled = true; continue }
            }
            if down {
                guard source.isFirstResponder, source.forwardsCanvasKey(code,
                    command: !key.modifierFlags.intersection([.command, .control]).isEmpty) else { continue }
            } else if !keys.contains(code) { continue }
            let repeated = down && keys.contains(code)
            if down { keys.insert(code) } else { keys.remove(code) }
            view.canvases?.input(view, ["t": "key", "code": code, "key": key.characters.isEmpty ? KeyCodes.key(code) : key.characters, "down": down, "repeat": repeated], timestamp: press.timestamp)
            handled = true
        }
        return handled && !presses.contains { $0.key?.modifierFlags.contains(.command) == true }
    }
    /// Only the hit canvas yields the presenter's recognizers. The window's
    /// four-finger dev recognizers retain their own delegate and cancel normally.
    static func owns(_ hit: UIView?) -> Bool {
        guard let view = hit as? NodeView else { return false }
        return !view.disabled && !view.inert && (view.canvases?.wantsInput(view.id) == true || view.isSurfaceControl)
    }
}

extension NodeView {
    var canvasScale: CGFloat { metal?.metalLayer.contentsScale ?? window?.screen.scale ?? traitCollection.displayScale }
    func focusCanvas() -> Bool {
        guard canvases?.wantsInput(id) == true, canBecomeFirstResponder, window != nil else { return false }
        var ancestor: UIView? = self
        while let view = ancestor {
            if view.isHidden { return false }
            ancestor = view.superview
        }
        if !isFirstResponder { _ = becomeFirstResponder() }
        return isFirstResponder
    }
}

extension Agent {
    /// UIKit exposes no touch constructor. This is the raw seam, labelled recognized.
    func canvasTap(_ request: [String: Any]) -> [String: Any]? {
        let phase = request["phase"] as? String
        let continuing = phase != nil && phase != "down"
        if continuing, canvasContact == nil, contact != nil {
            contact = nil
            return ["error": "the contact's canvas is gone"]
        }
        guard let node = continuing ? canvasContact : view(request), (session.canvases.wantsInput(node.id) || node.isSurfaceControl),
              session.presenter.views[node.id] === node, continuing || (!node.disabled && !node.inert), let window = node.window else { return nil }
        if request["contextmenu"] != nil || request["dblclick"] != nil { return nil }
        let vp = session.presenter.viewport
        let b = box(node)
        let start = contact ?? CGPoint(x: b.midX, y: b.midY)
        let point = CGPoint(x: request["x"] as? Double ?? start.x + (request["dx"] as? Double ?? 0),
                            y: request["y"] as? Double ?? start.y + (request["dy"] as? Double ?? 0))
        guard point.x.isFinite, point.y.isFinite else { return ["error": "pointer needs finite coordinates"] }
        let inWindow = vp.convert(CGPoint(x: point.x + vp.contentOffset.x, y: point.y + vp.contentOffset.y), to: nil)
        if !continuing {
            guard let hit = window.hitTest(inWindow, with: nil), hit === node || (node.isSurfaceControl && hit.isDescendant(of: node)) else { return ["error":"control or canvas is covered"] }
        }
        let p = node.local(inWindow)
        let at = [Agent.r2(point.x), Agent.r2(point.y)]
        func send(_ phase: String) -> Bool {
            if node.isSurfaceControl { return node.control(phase, point: p) }
            // A move's motion is its position's change; a down or up has none.
            let from = phase == "move" ? canvasPoint ?? p : p
            canvasPoint = p
            return session.canvases.input(node, ["t": "pointer", "phase": phase, "id": 1, "x": p.x, "y": p.y, "dx": p.x - from.x, "dy": p.y - from.y, "kind": "touch", "buttons": phase == "up" || phase == "cancel" ? 0 : 1])
        }
        if let wheel = request["wheel"] as? [Double], wheel.count == 2 {
            guard wheel.allSatisfy(\.isFinite) else { return ["error": "wheel deltas must be finite"] }
            let ok = session.canvases.input(node, ["t": "wheel", "dx": wheel[0], "dy": wheel[1], "x": p.x, "y": p.y])
            return ok ? ["tapped": node.id, "wheel": wheel, "at": at, "delivery": "recognized"] : ["error": "surface refused wheel"]
        }
        if request["hover"] as? Bool == true {
            let from = canvasPoint ?? p
            canvasPoint = p
            let ok = session.canvases.input(node, ["t": "pointer", "phase": "move", "id": 1, "kind": "mouse", "buttons": 0, "x": p.x, "y": p.y, "dx": p.x - from.x, "dy": p.y - from.y])
            return ok ? ["tapped": node.id, "hover": true, "at": at, "delivery": "recognized"] : ["error": "surface refused pointer"]
        }
        if phase == nil {
            guard node.focusSurfacePointer(), send("down"), send("up") else { return ["error": "surface refused pointer"] }
            return ["tapped": node.id, "at": at, "delivery": "recognized"]
        }
        switch phase {
        case "down":
            guard contact == nil else { return ["error": "a contact is already down; up or cancel it first"] }
            guard node.focusSurfacePointer(), send("down") else { return ["error": "surface refused pointer"] }
            contact = point; canvasContact = node
            return ["contact": node.id, "phase": "down", "at": at, "delivery": "recognized"]
        case "move", "up", "cancel":
            guard contact != nil else { return ["error": "no contact is down"] }
            guard send(phase!) else { return ["error": "surface refused pointer"] }
            contact = phase == "move" ? point : nil
            if contact == nil { canvasContact = nil }
        case "hold":
            guard let previous = contact else { return ["error": "no accepted contact is down"] }
            if point != previous {
                guard send("move") else { return ["error": "surface refused pointer"] }
                contact = point
            }
        default: return ["error": "unknown pointer phase \(phase!)"]
        }
        return ["phase": phase!, "at": at, "delivery": "recognized"]
    }
}
#endif

#if os(iOS)
extension NodeView {
    func pressedControls(_ presses: Set<UIPress>, down: Bool) -> Set<UIPress> {
        Set(presses.filter { press in
            guard let key=press.key, key.modifierFlags.intersection([.command,.control]).isEmpty else {return true}
            return presenter?.session?.canvases.pressedControlKey(KeyCodes.hid(key.keyCode.rawValue),down:down,timestamp:press.timestamp) != true
        })
    }
}
#endif

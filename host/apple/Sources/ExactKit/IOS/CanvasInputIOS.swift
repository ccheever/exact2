#if os(iOS)
import UIKit

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
        if let inactive { NotificationCenter.default.removeObserver(inactive) }
        view?.isMultipleTouchEnabled = multiple
    }
    private static func hasFocus(_ view: UIView) -> Bool { view.isFirstResponder || view.subviews.contains(where: hasFocus) }
    func blur() {
        touches.removeAll(); keys.removeAll()
        if let view { view.canvases?.input(view, ["t": "blur"]) }
    }
    func touches(_ values: Set<UITouch>, phase: String, source: NodeView) -> Bool {
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
            view.canvases?.input(view, ["t": "pointer", "phase": phase, "id": id, "x": p.x, "y": p.y, "kind": "touch", "buttons": phase == "up" || phase == "cancel" ? 0 : 1], timestamp: touch.timestamp)
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
    var canvasScale: CGFloat { metal?.layer.contentsScale ?? window?.screen.scale ?? traitCollection.displayScale }
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
            return session.canvases.input(node, ["t": "pointer", "phase": phase, "id": 1, "x": p.x, "y": p.y, "kind": "touch", "buttons": phase == "up" || phase == "cancel" ? 0 : 1])
        }
        if let wheel = request["wheel"] as? [Double], wheel.count == 2 {
            guard wheel.allSatisfy(\.isFinite) else { return ["error": "wheel deltas must be finite"] }
            let ok = session.canvases.input(node, ["t": "wheel", "dx": wheel[0], "dy": wheel[1], "x": p.x, "y": p.y])
            return ok ? ["tapped": node.id, "wheel": wheel, "at": at, "delivery": "recognized"] : ["error": "surface refused wheel"]
        }
        if request["hover"] as? Bool == true {
            let ok = session.canvases.input(node, ["t": "pointer", "phase": "move", "id": 1, "kind": "mouse", "buttons": 0, "x": p.x, "y": p.y])
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

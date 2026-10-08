// What the core's surface glue asks of this session's surfaces (LLP 1047.001
// D4; `ExactKit/SurfaceInput.swift`): its controls and the agent's world.
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import ExactKit

extension CanvasesHost {
    /// Whether a surface control's contact is `node`'s.
    func ownsControl(_ node: UInt32) -> Bool {
        entries.values.contains { $0.controls.values.contains { $0.node == node } }
    }

    /// Cancel every contact `node` holds on a surface.
    func cancelControls(of node: UInt32) {
        for e in entries.values {
            guard let m = e.module else { continue }
            for (contact, owner) in e.controls where owner.node == node {
                e.controls.removeValue(forKey: contact)
                _ = input(e, m, ["t":"control", "name":owner.name, "phase":"cancel", "id":contact, "x":owner.position.x, "y":owner.position.y])
            }
        }
    }

    /// A surface control's phase from `node` (LLP 1046.002 §3).
    func control(_ node: NodeView, _ phase: String, id contact: Int, point: CGPoint, timestamp: Double?) -> Bool {
        guard !modules.isEmpty else { return false }
        let entry: Entry?
        if phase == "down" {
            guard let name = node.props["action"], !node.disabled, !node.inert, let canvas = node.inputCanvas, let e = live(canvas.id) else { return false }
            if e.controls[contact] != nil { return true }
            _ = node.focusSurfacePointer()
            e.controls[contact] = SurfaceControl(node:node.id, name:name, offset:node.convert(.zero, to:canvas), position:point)
            entry = e
        } else {
            // Prefer this node's captured owner, then the addressed canvas. A
            // cross-view release may use a unique contact, never dictionary order.
            let owners = entries.values.filter { $0.controls[contact] != nil }
            entry = owners.first { $0.controls[contact]?.node == node.id }
                ?? node.inputCanvas.flatMap { canvas in owners.first { $0.view === canvas } }
                ?? (owners.count == 1 ? owners[0] : nil)
        }
        guard let e = entry, let m = e.module, var owner = e.controls[contact] else { return false }
        let p = node.convert(point, to:e.view)
        owner.position = CGPoint(x:p.x-owner.offset.x, y:p.y-owner.offset.y)
        if ["up", "cancel"].contains(phase) { e.controls.removeValue(forKey:contact) }
        else { e.controls[contact] = owner }
        return input(e, m, ["t":"control", "name":owner.name, "phase":phase, "id":contact, "x":owner.position.x, "y":owner.position.y], timestamp:timestamp)
    }

    /// The agent's `world` op on a surface (LLP 1046.001).
    func world(_ agent: Agent, _ request: [String: Any]) -> [String: Any] {
        let id = request["id"] as? UInt32
        let missing: [String: Any] = ["error": "view \(request["id"] ?? "undefined") has no world"]
        guard let id, let e = live(id) else { return missing }
        let op = request["op"] as? String ?? ""
        if op == "screenshot", request["form"] as? String == "save" { return save(e) }
        if op == "focus" {
            guard e.wantsInput else { return ["error": "view \(id)'s surface does not take input"] }
            return ["ok": e.view.focusCanvas()]
        }
        guard ["layout", "state", "tree"].contains(op) else { return ["error": "world does not answer \(op)"] }
        var request = request
        let rect = agent.box(e.view)
        // A `layout` with a point and no entity is the pick (LLP 1046.001 D2): a form, not a ninth name.
        if op == "layout", request["entity"] == nil {
            if let x = request["x"] as? Double { request["x"] = x - rect.minX }
            if let y = request["y"] as? Double { request["y"] = y - rect.minY }
        }
        var reply = self.agent(id, request) ?? missing
        for key in ["entity", "hit"] {
            if var entity = reply[key] as? [String: Any], var screen = entity["screen"] as? [String: Any] {
                if let x = screen["x"] as? Double { screen["x"] = x + rect.minX }
                if let y = screen["y"] as? Double { screen["y"] = y + rect.minY }
                entity["screen"] = screen
                reply[key] = entity
            }
        }
        return reply
    }

    /// The agent's `type` on a surface's view.
    func type(_ agent: Agent, _ view: NodeView, _ request: [String: Any]) -> [String: Any] {
        guard let e = live(view.id), e.view === view else { return ["error": "view \(view.id) has no world"] }
        guard e.wantsInput else { return ["error": "view \(view.id)'s surface does not take input"] }
        guard let key = request["key"] as? String, let device = KeyCodes.device(key) else { return ["error": "canvas key needs a device code"] }
        let phase = request["phase"] as? String
        guard phase == nil || phase == "down" || phase == "up" else { return ["error": "key: not a phase: \(phase!)"] }
        guard view.focusCanvas() else { return ["error": "view \(view.id) could not take focus"] }
        if phase == "down", let token = request["releaseKey"] as? String, let module = e.module {
            let surface = e.id
            agent.keyReleases[token] = { [weak self, weak e] in
                let reply: [String: Any] = ["typed": view.id, "key": key, "phase": "up", "delivery": "recognized"]
                // Destruction removes the held device state too. Never send to a replacement.
                guard let self, let e, self.entries[view.id] === e, e.id == surface else { return reply }
                guard self.input(e, module, ["t": "key", "code": device.code, "key": device.key, "down": false, "repeat": false]) else {
                    return ["error": "view \(view.id)'s surface refused key release"]
                }
                return reply
            }
        }
        for step in phase.map({ [$0] }) ?? ["down", "up"] {
            guard input(view, ["t": "key", "code": device.code, "key": device.key, "down": step == "down", "repeat": false]) else {
                return ["error": "view \(view.id)'s surface refused input"]
            }
        }
        var reply: [String: Any] = ["typed": view.id, "key": key, "delivery": "recognized"]
        if let phase { reply["phase"] = phase }
        return reply
    }
}

// T3Module's timeline ops (T3Module.swift routes them): the transcript's sleep
// timer, Mermaid fences, a citation's jump and image chip accents.
import Foundation
import AppKit

extension T3Module {
    /// Timeline sleep, Mermaid layout, citation jumps and image chip accents.
    func timelineOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "timelineSleep" { return T3TimelineTurns.sleep(request) { reply.send($0) } }
        if request["op"] as? String == "mermaidRender" { DispatchQueue.main.async { [weak self] in self?.mermaid.perform(request) { reply.send($0) } }; return }
        if request["op"] as? String == "timelineRelease" { DispatchQueue.main.async { [weak self] in self?.turns.cancel(); reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": [:]]) }; return }
        if request["op"] as? String == "timelineJump" { DispatchQueue.main.async { [weak self] in self?.turns.jump(request) { reply.send($0) } }; return }
        if request["op"] as? String == "imageAccent" { return T3ImageAccent.perform(request) { reply.send($0) } } // Image chip accents (T3ImageAccent.swift, lane r4-timeline).
        next()
    }
}

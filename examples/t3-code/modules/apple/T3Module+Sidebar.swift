// T3Module's sidebar op (T3Module.swift routes it): `sidebarNotify`, the read again a sidebar change asks for, now or
// after `delay` ms. Data code asks it when it learns something after its answer returned (a search's server answers, a
// release lookup) or when the window must be drawn again as a notice or a pill expires; a data module's own timers end
// with its answer (LLP 1027.000 D1).
import Foundation

extension T3Module {
    func sidebarOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        guard request["op"] as? String == "sidebarNotify" else { return next() }
        let generation = request["generation"] as? Int ?? 0, changed = context.changed
        let delay = max(0, (request["delay"] as? Double ?? 0) / 1000)
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { changed("t3.status") }
        reply.send(["ok": true, "generation": generation, "value": [:]])
    }
}

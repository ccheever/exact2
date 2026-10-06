// T3Module's terminal drawer ops (T3Module.swift routes them; task terminal-drawer): the drawer's
// mounted threads' terminals keep their attach streams without a view (T3TerminalSessions.retain).
import Foundation

extension T3Module {
    /// `terminalRetain` { sessions: [JSON.stringify([environmentId, threadId, terminalId])] }.
    func terminalOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        guard request["op"] as? String == "terminalRetain" else { return next() }
        let keys = Set((request["sessions"] as? [Any] ?? []).compactMap { $0 as? String })
        DispatchQueue.main.async { [transport] in
            T3TerminalSessions.of(transport).retain(keys)
            reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": ["retained": keys.count]])
        }
    }
}

// T3Module's connection ops (T3Module.swift routes them): Add Environment → SSH
// and the wake that reconnects after sleep.
import Foundation
import AppKit

extension T3Module {
    /// SSH environments (T3Ssh.swift) and lane r10-connect's wake (R10Connect.swift).
    func connectionOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if let op = request["op"] as? String, op.hasPrefix("ssh") { return ssh.perform(request) { reply.send($0) } }
        if request["op"] as? String == "r10Wake" { return R10Connect.wake(request, changed: { [gate] in gate.changed($0) }) { reply.send($0) } } // lane r10-connect
        next()
    }
}

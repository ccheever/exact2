// T3Module's connection ops (T3Module.swift routes them): Add Environment → SSH
// with its password prompts, remote Open's editors and deep links, and the wake that
// reconnects after sleep.
import Foundation
import AppKit

extension T3Module {
    /// SSH environments (T3Ssh.swift) and lane r10-connect's wake (R10Connect.swift).
    func connectionOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if let op = request["op"] as? String, op.hasPrefix("ssh") { return ssh.perform(request) { reply.send($0) } }
        if let op = request["op"] as? String, op.hasPrefix("remoteEditors") { return T3RemoteEditors.perform(request, agent: context.agent) { reply.send($0) } } // remote Open (T3RemoteEditors.swift)
        if request["op"] as? String == "r10Wake" { return R10Connect.wake(request, changed: context.changed) { reply.send($0) } } // lane r10-connect
        next()
    }
}

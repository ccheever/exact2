// T3Module's embedded-server ops (T3Module.swift routes them; 20261005-embedded-server-runtime):
// the local backend's status for TypeScript. The backend itself is one per app
// (T3LocalBackend.swift): each session's module attaches at init and detaches at destroy.
import Foundation

extension T3Module {
    /// `localBackendStatus`: `{state, port, httpBaseUrl, wsBaseUrl, bearerReady, restartAttempt,
    /// nextRestartMs, lastExit, install, refused, …}`; `t3.local` announces each change.
    func localOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "localBackendStatus" {
            return reply.send(["ok": true, "generation": request["generation"] as? Int ?? 0, "value": T3LocalBackend.shared.statusValue()])
        }
        next()
    }

    func attachLocalBackend(_ context: ExactModuleContext) {
        T3LocalBackend.shared.attach(self, dataRoot: T3Storage.dataRoot(agent: context.agent, contextData: context.data), changed: context.changed)
    }
}

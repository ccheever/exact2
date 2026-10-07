// T3Module's embedded-server ops (T3Module.swift routes them; 20261005-embedded-server-runtime,
// 20261005-local-primary-environment): the local backend's status for TypeScript and the Local
// environment switch. The backend itself is one per app (T3LocalBackend.swift): each session's
// module attaches at init and detaches at destroy.
import AppKit

extension T3Module {
    /// `localBackendStatus`: `{state, enabled, port, httpBaseUrl, wsBaseUrl, bearerReady, environmentId,
    /// label, serverVersion, restartAttempt, nextRestartMs, lastExit, install, refused, …}`; `t3.local`
    /// announces each change. `localBackendSetEnabled {enabled}`: the switch's stopgap (stop, or start
    /// and wait until ready); it answers the status, or the reason it could not. `localBackendRetry` runs a
    /// failed runtime install again; `localBackendQuit` quits the app (the first-launch view's two buttons).
    func localOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        let generation = request["generation"] as? Int ?? 0
        switch request["op"] as? String {
        case "localBackendStatus":
            reply.send(["ok": true, "generation": generation, "value": T3LocalBackend.shared.statusValue()])
        case "localBackendSetEnabled":
            guard let enabled = request["enabled"] as? Bool else {
                return reply.send(["ok": false, "generation": generation, "error": T3Failure(kind: "Arguments", message: "localBackendSetEnabled requires enabled.").json])
            }
            T3LocalBackend.shared.setEnabled(enabled) { failure in
                if let failure { reply.send(["ok": false, "generation": generation, "error": T3Failure(kind: "LocalEnvironment", message: failure).json]) }
                else { reply.send(["ok": true, "generation": generation, "value": T3LocalBackend.shared.statusValue()]) }
            }
        case "localBackendRetry":
            // The first-launch view's Retry: the failed runtime install again (portable-app-download).
            T3LocalBackend.shared.retryInstall { failure in
                if let failure { reply.send(["ok": false, "generation": generation, "error": T3Failure(kind: "LocalEnvironment", message: failure).json]) }
                else { reply.send(["ok": true, "generation": generation, "value": T3LocalBackend.shared.statusValue()]) }
            }
        case "localBackendQuit":
            // The first-launch view's Quit: the app's own quit (the server stops in the sessions' teardown).
            reply.send(["ok": true, "generation": generation])
            DispatchQueue.main.async { NSApp.terminate(nil) }
        default: next()
        }
    }

    func attachLocalBackend(_ context: ExactModuleContext) {
        T3LocalBackend.shared.attach(self, dataRoot: T3Storage.dataRoot(agent: context.agent, contextData: context.data), changed: context.changed)
    }
}

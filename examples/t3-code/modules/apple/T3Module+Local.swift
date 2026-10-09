// T3Module's embedded-server ops (T3Module.swift routes them; 20261005-embedded-server-runtime,
// 20261005-local-primary-environment): the local backend's status for TypeScript and the Local
// environment switch. The backend itself is one per app (T3LocalBackend.swift): each session's
// module attaches at init and detaches at destroy.
import Foundation

extension T3Module {
    /// `localBackendStatus`: `{state, enabled, port, httpBaseUrl, wsBaseUrl, bearerReady, environmentId,
    /// label, serverVersion, restartAttempt, nextRestartMs, lastExit, install, refused, …}`; `t3.local`
    /// announces each change; its `desktopSettings` are the five keys of desktop-settings.json this app
    /// changes, written by `desktopSettingsSet`. `localBackendSetEnabled {enabled}`: the switch's stopgap (stop, or start
    /// and wait until ready); it answers the status, or the reason it could not. `localNetworkFacts`,
    /// `localBackendRestart {host, tailscaleServe*}` and `localAccess {method, path, body}`: Network access,
    /// Tailscale HTTPS and the Authorized clients (20261005-this-machine-network-access).
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
        // Decision U7: the desktop settings file (T3DesktopSettings.swift); a write failure is the reference's
        // DesktopSettingsWriteError message.
        case "desktopSettingsSet":
            do { reply.send(["ok": true, "generation": generation, "value": try T3LocalBackend.shared.setDesktopSettings(request)]) }
            catch let failure as T3Failure { reply.send(["ok": false, "generation": generation, "error": failure.json]) }
            catch let failure as T3DesktopSettingsWriteError { reply.send(["ok": false, "generation": generation, "error": T3Failure(kind: "DesktopSettings", message: failure.message).json]) }
            catch { reply.send(["ok": false, "generation": generation, "error": T3Failure(kind: "DesktopSettings", message: "\(error)").json]) }
        // 20261005-this-machine-network-access (T3LocalNetwork.swift, T3LocalBackend.swift).
        case "localNetworkFacts":
            let tailscale = request["tailscale"] as? Bool == true, probe = request["probe"] as? String ?? "", refresh = request["refresh"] as? Bool == true
            DispatchQueue.global(qos: .userInitiated).async {
                reply.send(["ok": true, "generation": generation, "value": T3LocalNetwork.shared.facts(tailscale: tailscale, probe: probe, refresh: refresh)])
            }
        case "localBackendRestart":
            guard let host = request["host"] as? String, ["127.0.0.1", "0.0.0.0"].contains(host), let serve = request["tailscaleServeEnabled"] as? Bool else {
                return reply.send(["ok": false, "generation": generation, "error": T3Failure(kind: "Arguments", message: "localBackendRestart requires host and tailscaleServeEnabled.").json])
            }
            let exposure = T3LocalExposure(host: host, tailscaleServeEnabled: serve, tailscaleServePort: T3LocalExposure.normalizedPort(request["tailscaleServePort"]))
            T3LocalBackend.shared.restart(exposure: exposure) { failure in
                if let failure { reply.send(["ok": false, "generation": generation, "error": T3Failure(kind: "LocalEnvironment", message: failure).json]) }
                else { reply.send(["ok": true, "generation": generation, "value": T3LocalBackend.shared.statusValue()]) }
            }
        case "localAccess":
            guard let method = request["method"] as? String, let path = request["path"] as? String else {
                return reply.send(["ok": false, "generation": generation, "error": T3Failure(kind: "Arguments", message: "localAccess requires method and path.").json])
            }
            T3LocalBackend.shared.access(method: method, path: path, body: request["body"]) { result in
                switch result {
                case let .success(value): reply.send(["ok": true, "generation": generation, "value": value])
                case let .failure(failure): reply.send(["ok": false, "generation": generation, "error": failure.json])
                }
            }
        default: next()
        }
    }

    func attachLocalBackend(_ context: ExactModuleContext) {
        T3LocalBackend.shared.attach(self, dataRoot: T3Storage.dataRoot(agent: context.agent, contextData: context.data), changed: context.changed)
    }
}

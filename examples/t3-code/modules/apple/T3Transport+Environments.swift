// T3Transport's saved-environment ops (T3Transport.perform routes them): the
// saved list and its preferences, enabling and forgetting one environment, and
// pairing another without touching the active connection. All run on the
// transport's queue.
import Foundation

extension T3Transport {
    /// Saved environments and pairing; false for an op that is not one of them.
    func environmentOps(_ request: [String: Any], completion: @escaping Completion) throws -> Bool {
        switch request["op"] as? String {
        case "environments": finish(completion, value: ["saved": savedEnvironments.all])
        case "connectionPreferences": finish(completion, value: ["text": savedEnvironments.preferences])
        case "setConnectionPreferences":
            guard let text = request["text"] as? String else { throw arguments("setConnectionPreferences requires text.") }
            try savedEnvironments.setPreferences(text); finish(completion, value: ["text": text])
        case "setEnvironmentEnabled":
            guard let raw = request["origin"] as? String, let environment = request["environmentId"] as? String, !environment.isEmpty,
                  let enabled = request["enabled"] as? Bool else { throw arguments("setEnvironmentEnabled requires origin, environmentId and enabled.") }
            let saved = try T3Endpoint.origin(raw)
            guard savedEnvironments.setEnabled(origin: saved.absoluteString, environment: environment, enabled: enabled) else {
                throw T3Failure(kind: "Missing", message: "That environment is no longer saved on this device.")
            }
            finish(completion, value: ["saved": savedEnvironments.all])
        case "forgetEnvironment":
            // Forget one saved environment's pairing on this device; the
            // active connection closes only when it is that environment.
            guard let raw = request["origin"] as? String, let environment = request["environmentId"] as? String, !environment.isEmpty else {
                throw arguments("forgetEnvironment requires origin and environmentId.")
            }
            let saved = try T3Endpoint.origin(raw)
            let gone = savedEnvironments.forget(origin: saved.absoluteString, environment: environment)
            for owner in T3SavedEnvironments.credentialOrigins(gone) + [saved.absoluteString] { try credentials.forget(origin: owner, environment: environment) }
            let focusedId = descriptor["environmentId"] as? String ?? ""
            // One entry per environment: its focus may be any of its routes.
            let focused = focusedId == environment || (focusedId.isEmpty && (origin == saved || routes.environmentId == environment))
            if focused {
                wantsConnection = false; reconnect?.cancel(); reconnect = nil; routes.stop()
                retire(T3Failure(kind: "Disconnected", message: "Disconnected from the server.", uncertain: true))
                generation += 1; inbox.reset(); token = ""
                setStatus("disconnected", "Disconnected.")
            }
            forgotten(saved, wasFocused: focused)
            finish(completion, value: status())
        case "pairEnvironment": try pairEnvironment(request, completion: completion)
        default: return false
        }
        return true
    }

    /// Pair another environment without touching the active connection: read its
    /// descriptor, exchange the one-time credential, and keep the token in Keychain.
    func pairEnvironment(_ request: [String: Any], completion: @escaping Completion) throws {
        let target = try T3Endpoint.origin(request["origin"] as? String ?? "")
        let credential = try T3Endpoint.credential(request["credential"] as? String ?? "", at: target)
        guard !credential.isEmpty else { throw T3Failure(kind: "Credential", message: "Enter a pairing code.") }
        let secrets = [credential]
        let scrub = { (text: String) -> String in secrets.reduce(self.clean(text)) { $0.replacingOccurrences(of: $1, with: "[redacted]") } }
        func send(_ path: String, body: Data?, then: @escaping ([String: Any]) -> Void) {
            do {
                var req = URLRequest(url: try T3Endpoint.path(path, at: target))
                req.httpMethod = body == nil ? "GET" : "POST"; req.httpBody = body
                req.setValue("2", forHTTPHeaderField: "x-t3-orchestration-protocol")
                req.setValue("application/json", forHTTPHeaderField: "Accept")
                if body != nil { req.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type") }
                session.dataTask(with: req) { [weak self] data, response, error in
                    guard let self else { return }
                    self.queue.async {
                        guard self.alive else { return self.finish(completion, failure: T3Failure(kind: "Closed", message: "The window was closed.")) }
                        if let error { return self.finish(completion, failure: T3Failure(kind: "Network", message: scrub(error.localizedDescription))) }
                        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
                        let bytes = data ?? Data()
                        let decoded = bytes.count <= T3Wire.maximumBytes ? (try? JSONSerialization.jsonObject(with: bytes)) as? [String: Any] : nil
                        guard (200..<300).contains(status), let decoded else {
                            let reason = T3RemoteAuth.failureMessage(decoded, status: status)
                            return self.finish(completion, failure: T3Failure(kind: [401, 403].contains(status) ? "Authentication" : "HTTP", message: scrub(reason)))
                        }
                        then(decoded)
                    }
                }.resume()
            } catch { finish(completion, failure: failure(error)) }
        }
        send("/.well-known/t3/environment", body: nil) { [self] descriptor in
            guard let environment = descriptor["environmentId"] as? String, !environment.isEmpty else {
                return finish(completion, failure: T3Failure(kind: "Protocol", message: "The server did not identify its environment."))
            }
            // Adding a route to a saved machine: checked before the one-time code is spent (onboarding.ts).
            if let expected = request["expectedEnvironmentId"] as? String, !expected.isEmpty, environment != expected {
                let place = request["ssh"] as? Bool == true ? "host" : "address"
                return finish(completion, failure: T3Failure(kind: "Pairing", message: "That \(place) reaches \(descriptor["label"] as? String ?? "another environment"), a different machine. Add it as its own environment instead."))
            }
            // The message names the direction (compatibility.ts); T3Fleet's fleetOutdatedPair saves an
            // outdated host that can update itself, so this one never exchanges the credential.
            if let problem = T3Compatibility.problem(descriptor) { return finish(completion, failure: T3Failure(kind: "Protocol", message: problem.message)) }
            let body = T3RemoteAuth.exchangeForm(credential: credential, scope: request["scope"] as? String ?? "")
            send("/oauth/token", body: body) { [self] grant in
                guard grant["token_type"] as? String == "Bearer", let access = grant["access_token"] as? String, !access.isEmpty else {
                    return finish(completion, failure: T3Failure(kind: "Protocol", message: "The server did not issue a bearer access token."))
                }
                do { try credentials.save(access, origin: target.absoluteString, environment: environment) }
                catch { return finish(completion, failure: failure(error)) }
                savedEnvironments.remember(origin: target.absoluteString, descriptor: descriptor)
                finish(completion, value: ["origin": target.absoluteString, "environmentId": environment, "label": descriptor["label"] as? String ?? ""])
            }
        }
    }
}

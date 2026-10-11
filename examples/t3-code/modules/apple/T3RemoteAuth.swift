import Foundation

/// The pairing token exchange every remote path shares (T3 Code MIT, see LICENSE-T3; reference
/// 1e2ecbd975: packages/client-runtime/src/authorization/remote.ts bootstrapRemoteBearerSession,
/// packages/client-runtime/src/connection/errors.ts mapRemoteEnvironmentError).
/// The scope names live in TypeScript (`remote-scopes.ts`); this file sends the `scope` it is given.
enum T3RemoteAuth {
    /// `clientMetadataTokenExchangeFields`: the label, device type and OS a session is listed with.
    struct Client {
        var label: String
        var deviceType = "desktop"
        var os: String?
        /// What this app sends for a paired environment.
        static let remote = Client(label: "Exact T3 for Mac", os: "macos")
        /// The embedded server's own session: DesktopLocalEnvironmentAuth.ts sends only these two.
        static let localDesktop = Client(label: "T3 Code Desktop")
    }

    /// The `/oauth/token` form body. `scope` is sent only when the caller named one, as
    /// remote.ts spreads `scope` only when `scopes` is given (the embedded server's exchange
    /// names none, so it is granted every scope).
    static func exchangeForm(credential: String, scope: String = "", client: Client = .remote) -> Data {
        var fields = [
            "grant_type": "urn:ietf:params:oauth:grant-type:token-exchange",
            "subject_token": credential,
            "subject_token_type": "urn:t3:params:oauth:token-type:environment-bootstrap",
            "requested_token_type": "urn:ietf:params:oauth:token-type:access_token",
            "client_label": client.label, "client_device_type": client.deviceType,
        ]
        if let os = client.os { fields["client_os"] = os }
        if !scope.isEmpty { fields["scope"] = scope }
        return T3Endpoint.form(fields)
    }

    /// The text for a refused request: the server's own message, then mapRemoteEnvironmentError's
    /// detail for an invalid credential or a scope the link does not grant (only the token exchange
    /// answers `scope_not_granted` / `invalid_scope`), then a pull request error's own sentence (its
    /// `message` is a getter the schema does not send, as on the RPC path, T3Protocol `typed`), then the
    /// HTTP status. realinput-1010e-followups RE-5: `POST /api/pull-requests/diff` answers 503 with
    /// `{ _tag: "PullRequestUnavailableError", reason, provider }`, which the reference's HttpApi client
    /// decodes and shows by its message ("Change requests cannot be browsed for this project's host yet.").
    static func failureMessage(_ decoded: [String: Any]?, status: Int) -> String {
        if let message = decoded?["message"] as? String { return message }
        let text = { (key: String) in (decoded?[key] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? "" }
        switch text("reason") {
        case "invalid_credential": return "The environment credential is invalid."
        case "scope_not_granted", "invalid_scope": return "The environment rejected the authentication request."
        default: break
        }
        switch text("_tag") {
        case "PullRequestUnavailableError":
            let sentence = T3PullRequestErrors.unavailable(reason: text("reason"), provider: text("provider"))
            if !sentence.isEmpty { return sentence }
        case "PullRequestOperationError" where !text("detail").isEmpty: return "Pull request operation \(text("operation")) failed: \(text("detail"))"
        default: break
        }
        return "The server returned HTTP \(status)."
    }
}

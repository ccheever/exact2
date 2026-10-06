import Foundation

/// The pairing token exchange every remote path shares (T3 Code MIT, see LICENSE-T3; reference
/// 1e2ecbd975: packages/client-runtime/src/authorization/remote.ts bootstrapRemoteBearerSession,
/// packages/client-runtime/src/connection/errors.ts mapRemoteEnvironmentError).
/// The scope names live in TypeScript (`remote-scopes.ts`); this file sends the `scope` it is given.
enum T3RemoteAuth {
    /// The `/oauth/token` form body. `scope` is sent only when the caller named one, as
    /// remote.ts spreads `scope` only when `scopes` is given.
    static func exchangeForm(credential: String, scope: String) -> Data {
        var fields = [
            "grant_type": "urn:ietf:params:oauth:grant-type:token-exchange",
            "subject_token": credential,
            "subject_token_type": "urn:t3:params:oauth:token-type:environment-bootstrap",
            "requested_token_type": "urn:ietf:params:oauth:token-type:access_token",
            "client_label": "Exact T3 for Mac", "client_device_type": "desktop", "client_os": "macos",
        ]
        if !scope.isEmpty { fields["scope"] = scope }
        return T3Endpoint.form(fields)
    }

    /// The text for a refused request: the server's own message, then mapRemoteEnvironmentError's
    /// detail for an invalid credential or a scope the link does not grant (only the token exchange
    /// answers `scope_not_granted` / `invalid_scope`), then the HTTP status.
    static func failureMessage(_ decoded: [String: Any]?, status: Int) -> String {
        if let message = decoded?["message"] as? String { return message }
        switch decoded?["reason"] as? String ?? "" {
        case "invalid_credential": return "The environment credential is invalid."
        case "scope_not_granted", "invalid_scope": return "The environment rejected the authentication request."
        default: break
        }
        return "The server returned HTTP \(status)."
    }
}

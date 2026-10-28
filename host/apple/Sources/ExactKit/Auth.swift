// `openAuthSession` (LLP 1069.006 D3): one sign-in sheet per `exact-auth:`
// request, in ExactKit so an embedder (LLP 1031) gets it with the session.
// The runner rules first (grants, the one-session rule, supersession); this
// keeps each request's ticket until its sheet's completion runs, and reports
// what came back. The sheet itself, `ASWebAuthenticationSession`, is the
// `ExactAuthSession` module's (LLP 1047.001.000 E): AuthenticationServices
// is linked only by an app whose grants name `auth.session`. The callback's
// scheme, host, path and `state` are checked in Rust
// (`exact_runner::auth::accept`), since `.customScheme` matches the scheme
// only. Under the agent nothing opens: the request is held for `type @t`.
import Foundation
#if canImport(UIKit)
import UIKit
package typealias AuthAnchor = UIWindow
#else
import AppKit
package typealias AuthAnchor = NSWindow
#endif

/// What the core asks of the sign-in sheet's module.
package protocol AuthSessionCapability: AnyObject {
    /// Open the sheet for `url`, returning to `callback`. `done` runs once,
    /// on the main queue, unless the sheet is cancelled first; a sheet that
    /// cannot open is the failure, and `done` never runs.
    func open(url: URL, callback: URLComponents, ephemeral: Bool, anchor: @escaping () -> AuthAnchor?,
              done: @escaping (Result<URL, AuthFailure>) -> Void) -> Result<AuthSheet, AuthFailure>
}

/// An open sheet: releasing it cancels it too.
package protocol AuthSheet: AnyObject {
    func cancel()
}

/// An outcome without a callback URL: LLP 1069.006 D1's status and message.
package struct AuthFailure: Error {
    package let status: Int
    package let message: String
    package init(status: Int, message: String) { self.status = status; self.message = message }
}

/// The sign-in sheet's module, installed by the composition.
package enum AuthSessionLink {
    package static var installed: AuthSessionCapability?
}

final class AuthSessions {
    /// Open sheets by ticket, kept until each one's completion runs (or
    /// the runner lets go of it).
    var live: [UInt64: AuthSheet] = [:]
}

extension ExactSession {
    /// A batch's `auth` op: open, or cancel one the runner let go of.
    func authOp(_ op: [String: Any]) {
        guard let ticket = (op["ticket"] as? NSNumber)?.uint64Value else { return }
        if op["cancel"] as? Bool == true {
            // Supersession or teardown: cancel, and drop the late completion.
            authSessions.live.removeValue(forKey: ticket)?.cancel()
            return
        }
        // Under the agent the request is held (D7) — unless the drive asked
        // for the real device (`EXACT_DEVICE=real`, LLP 1069.007 D8), which
        // also restores OS entropy (`exact_data::crypto`).
        if ExactEnv.agentMode && ExactEnv.environment["EXACT_DEVICE"] != "real" {
            runtime.auth(["op": "hold", "ticket": ticket])
            return
        }
        #if os(tvOS)
        authDone(ticket, AuthFailure(status: 501, message: "tvOS has no web authentication session"))
        #else
        guard let urlText = op["url"] as? String, let url = URL(string: urlText),
              let callback = op["callback"] as? String, let parts = URLComponents(string: callback), parts.scheme != nil
        else { return authDone(ticket, AuthFailure(status: 502, message: "the session's URL or callback is unreadable")) }
        // A loopback callback (LLP 1133.005 D9): the default browser and the
        // listener's receiver on macOS; iOS waits on a device measurement (§8).
        if let loopback = op["loopback"] as? [String: Any] {
            #if os(macOS)
            let opened = authLoopback(ticket, url: url, state: op["state"] as? String ?? "", ephemeral: op["ephemeral"] as? Bool ?? false,
                                      loopback: loopback, done: { [weak self] in self?.authCompleted(ticket, $0) })
            switch opened {
            case .success(let sheet): authSessions.live[ticket] = sheet
            case .failure(let failure): authDone(ticket, failure)
            }
            #else
            authDone(ticket, AuthFailure(status: 501, message: "a loopback callback is macOS's; iOS's waits on a device measurement"))
            #endif
            return
        }
        // An app whose grants never named `auth.session` links no sheet; a
        // development plan that names it later needs a rebuild.
        guard let link = AuthSessionLink.installed else {
            return authDone(ticket, AuthFailure(status: 501, message: "this build links no sign-in sheet (auth_session): rebuild with the `auth.session` grant"))
        }
        let opened = link.open(url: url, callback: parts, ephemeral: op["ephemeral"] as? Bool ?? false,
                               anchor: { [weak self] in self?.presenter.shareAnchor(nil).0?.window },
                               done: { [weak self] in self?.authCompleted(ticket, $0) })
        switch opened {
        case .success(let sheet): authSessions.live[ticket] = sheet
        case .failure(let failure): authDone(ticket, failure)
        }
        #endif
    }

    private func authCompleted(_ ticket: UInt64, _ outcome: Result<URL, AuthFailure>) {
        // A sheet the runner let go of was cancelled; its completion is dropped.
        guard authSessions.live.removeValue(forKey: ticket) != nil else { return }
        switch outcome {
        case .success(let url): runtime.auth(["op": "done", "ticket": ticket, "url": url.absoluteString])
        case .failure(let failure): authDone(ticket, failure)
        }
    }

    private func authDone(_ ticket: UInt64, _ failure: AuthFailure) {
        runtime.auth(["op": "done", "ticket": ticket, "status": failure.status, "message": failure.message])
    }

    /// Teardown or reload: every open sheet ends with it (D3).
    func cancelAuthSessions() {
        let live = authSessions.live
        authSessions.live = [:]
        for sheet in live.values { sheet.cancel() }
    }
}

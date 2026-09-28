// `openAuthSession` (LLP 1069.006 D3): one ASWebAuthenticationSession per
// `exact-auth:` request, in ExactKit so an embedder (LLP 1031) gets it with
// the session. The runner rules first (grants, the one-session rule,
// supersession); this opens the platform's sheet off this session's own
// window, retains it until its completion runs, and reports what came back.
// The callback's scheme, host, path and `state` are checked in Rust
// (`exact_runner::auth::accept`), since `.customScheme` matches the scheme
// only. Under the agent nothing opens: the request is held for `type @t`.
import AuthenticationServices
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

final class AuthSessions: NSObject, ASWebAuthenticationPresentationContextProviding {
    weak var owner: ExactSession?
    /// Live sessions by ticket; releasing one cancels it, so it is kept
    /// until its completion handler runs (or the runner lets go of it).
    var live: [UInt64: ASWebAuthenticationSession] = [:]

    func presentationAnchor(for _: ASWebAuthenticationSession) -> ASPresentationAnchor {
        owner?.presenter.shareAnchor(nil).0?.window ?? ASPresentationAnchor()
    }
}

extension ExactSession {
    /// A batch's `auth` op: open, or cancel one the runner let go of.
    func authOp(_ op: [String: Any]) {
        guard let ticket = (op["ticket"] as? NSNumber)?.uint64Value else { return }
        let sessions = authSessions
        if op["cancel"] as? Bool == true {
            // Supersession or teardown: cancel, and drop the late completion.
            sessions.live.removeValue(forKey: ticket)?.cancel()
            return
        }
        // Under the agent the request is held (D7) — unless the drive asked
        // for the real device (`EXACT_DEVICE=real`, LLP 1069.007 D8), which
        // also restores OS entropy (`exact_data::crypto`).
        if ExactEnv.agentMode && ExactEnv.environment["EXACT_DEVICE"] != "real" {
            runtime.auth(["op": "hold", "ticket": ticket])
            return
        }
        guard let urlText = op["url"] as? String, let url = URL(string: urlText),
              let callback = op["callback"] as? String, let parts = URLComponents(string: callback),
              let scheme = parts.scheme?.lowercased()
        else { return authDone(ticket, status: 502, message: "the session's URL or callback is unreadable") }
        let done: ASWebAuthenticationSession.CompletionHandler = { [weak self] url, error in
            DispatchQueue.main.async { self?.authCompleted(ticket, url: url, error: error) }
        }
        let session: ASWebAuthenticationSession
        if scheme == "https" {
            guard #available(iOS 17.4, macOS 14.4, *), let host = parts.host else {
                return authDone(ticket, status: 501, message: "an https callback needs iOS 17.4 or macOS 14.4")
            }
            session = ASWebAuthenticationSession(url: url, callback: .https(host: host, path: parts.path), completionHandler: done)
        } else if #available(iOS 17.4, macOS 14.4, *) {
            session = ASWebAuthenticationSession(url: url, callback: .customScheme(scheme), completionHandler: done)
        } else {
            session = ASWebAuthenticationSession(url: url, callbackURLScheme: scheme, completionHandler: done)
        }
        session.prefersEphemeralWebBrowserSession = op["ephemeral"] as? Bool ?? false
        session.presentationContextProvider = sessions
        sessions.live[ticket] = session
        if !session.start() {
            sessions.live[ticket] = nil
            authDone(ticket, status: 502, message: "the session could not start (no window)")
        }
    }

    private func authCompleted(_ ticket: UInt64, url: URL?, error: Error?) {
        // A session the runner let go of was cancelled; its completion is dropped.
        guard authSessions.live.removeValue(forKey: ticket) != nil else { return }
        if let url {
            runtime.auth(["op": "done", "ticket": ticket, "url": url.absoluteString])
        } else if (error as? ASWebAuthenticationSessionError)?.code == .canceledLogin {
            authDone(ticket, status: 499, message: "cancelled")
        } else {
            authDone(ticket, status: 502, message: error?.localizedDescription ?? "the session ended without a callback")
        }
    }

    private func authDone(_ ticket: UInt64, status: Int, message: String) {
        runtime.auth(["op": "done", "ticket": ticket, "status": status, "message": message])
    }

    /// Teardown or reload: every live session ends with it (D3).
    func cancelAuthSessions() {
        let live = authSessions.live
        authSessions.live = [:]
        for session in live.values { session.cancel() }
    }
}

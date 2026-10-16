// @ref LLP 1069.006 D3, LLP 1047.001.000 E — the sign-in sheet,
// `ASWebAuthenticationSession`, for `openAuthSession`. AuthenticationServices
// is in its own module because linking it costs every iOS launch, before
// `main`, the loading of ~300 more images (~1 s in the simulator). An app's
// composition links this module only when its grants name `auth.session`
// (`auth_session`) and installs it before any session is made; the core
// (`Auth.swift`) keeps the tickets, the agent's hold and the outcomes, and
// asks this module only to open and cancel the sheet. tvOS has no sheet, so
// installing it there does nothing.
import ExactKit

public enum ExactAuthSession {
    public static func install() {
        #if !os(tvOS)
        AuthSessionLink.installed = AuthSessionHost()
        #endif
    }
}

#if !os(tvOS)
import AuthenticationServices

final class AuthSessionHost: AuthSessionCapability {
    func open(url: URL, callback: URLComponents, ephemeral: Bool, anchor: @escaping () -> AuthAnchor?,
              done: @escaping (Result<URL, AuthFailure>) -> Void) -> Result<AuthSheet, AuthFailure> {
        guard let scheme = callback.scheme?.lowercased() else {
            return .failure(AuthFailure(status: 502, message: "the session's URL or callback is unreadable"))
        }
        let sheet = Sheet(anchor)
        let completion: ASWebAuthenticationSession.CompletionHandler = { url, error in
            DispatchQueue.main.async {
                if let url {
                    done(.success(url))
                } else if (error as? ASWebAuthenticationSessionError)?.code == .canceledLogin {
                    done(.failure(AuthFailure(status: 499, message: "cancelled")))
                } else {
                    done(.failure(AuthFailure(status: 502, message: error?.localizedDescription ?? "the session ended without a callback")))
                }
            }
        }
        let session: ASWebAuthenticationSession
        if scheme == "https" {
            guard #available(iOS 17.4, macOS 14.4, *), let host = callback.host else {
                return .failure(AuthFailure(status: 501, message: "an https callback needs iOS 17.4 or macOS 14.4"))
            }
            session = ASWebAuthenticationSession(url: url, callback: .https(host: host, path: callback.path), completionHandler: completion)
        } else if #available(iOS 17.4, macOS 14.4, *) {
            session = ASWebAuthenticationSession(url: url, callback: .customScheme(scheme), completionHandler: completion)
        } else {
            session = ASWebAuthenticationSession(url: url, callbackURLScheme: scheme, completionHandler: completion)
        }
        session.prefersEphemeralWebBrowserSession = ephemeral
        // The session holds its provider weakly; the sheet holds both.
        session.presentationContextProvider = sheet
        sheet.session = session
        guard session.start() else {
            return .failure(AuthFailure(status: 502, message: "the session could not start (no window)"))
        }
        return .success(sheet)
    }

    private final class Sheet: NSObject, AuthSheet, ASWebAuthenticationPresentationContextProviding {
        let anchor: () -> AuthAnchor?
        var session: ASWebAuthenticationSession?
        init(_ anchor: @escaping () -> AuthAnchor?) { self.anchor = anchor }
        func cancel() { session?.cancel() }
        func presentationAnchor(for _: ASWebAuthenticationSession) -> ASPresentationAnchor {
            anchor() ?? ASPresentationAnchor()
        }
    }
}
#endif

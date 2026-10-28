// @exact/in-app-browser on iOS and macOS (LLP 1133.024 D11–D13): Safari's
// view over the app on iOS, the default browser on the Mac, where AppKit has
// no in-app browser and a sheet with a web view would be the tell.
//
// - The embedder decides first (`context.leave`, D22 item 7): a refusal
//   answers `NotAllowedError`, and nothing opens.
// - iOS: one `SFSafariViewController` at a time, presented from the session's
//   window; its delegate's `safariViewControllerDidFinish` answers
//   `dismissed`. A call while one is shown is `InvalidStateError`. A call
//   let go dismisses the view it presented (`cancel`).
// - macOS: `NSWorkspace.open`, answering `opened`.
// - Under the agent nothing opens: the runner holds the send as a `browser`
//   ticket, whose tap arrives as `preferred` and is the answer. The journal
//   names the URL's origin, never its path.
// - Tier 3: `InAppBrowserModule.safariViewController` is handed each view
//   after the module configures it and before it is presented; the
//   delegate and the presentation stay the module's.
import ExactPackage
import Foundation
#if os(iOS)
import SafariServices
import UIKit
#elseif os(macOS)
import AppKit
#endif

public final class InAppBrowserModule: NSObject, ExactPackage {
    #if os(iOS)
    /// The app's hatch (D13): each view, configured, before it is presented.
    public static var safariViewController: ((SFSafariViewController) -> Void)?
    private var shown: (view: SFSafariViewController, request: Int?, reply: ([String: Any]) -> Void)?
    #endif
    let context: ExactPackageContext

    public init(context: ExactPackageContext) { self.context = context }

    private func say(_ line: String) { context.log("in-app-browser: \(line)") }

    private static var overApp: Bool {
        #if os(iOS)
        true
        #else
        false
        #endif
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "inAppBrowserStatus": reply(Self.status)
        case "openBrowser": open(args, tap: (preferred as? [String: Any])?["tap"] as? String, reply)
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name.hasPrefix("inAppBrowserStatus") ? Self.status : ["error": "NotSupportedError"]
    }

    static let status: [String: Any] = ["available": true, "overApp": overApp]

    /// An absolute http or https URL, or nil (D11: anything else is the app's `openURL`).
    static func web(_ text: String?) -> URL? {
        guard let text, let url = URL(string: text), let scheme = url.scheme?.lowercased(), ["http", "https"].contains(scheme), url.host != nil else { return nil }
        return url
    }

    private static func origin(_ url: URL) -> String {
        "\(url.scheme ?? "")://\(url.host ?? "")\(url.port.map { ":\($0)" } ?? "")"
    }

    private func open(_ args: [String: Any], tap: String?, _ reply: @escaping ([String: Any]) -> Void) {
        guard let url = Self.web(args["url"] as? String) else {
            return reply(["error": "TypeError", "message": "openBrowser takes an absolute http or https URL; open any other with openURL"])
        }
        if context.agent {
            let action = tap ?? (Self.overApp ? "dismissed" : "opened")
            say("\(action == "dismissed" ? "dismissed" : "opened") \(Self.origin(url))")
            return reply(["action": action])
        }
        guard context.leave(url.absoluteString, how: Self.overApp ? "browser" : "external") else {
            say("the embedder kept the app from opening \(Self.origin(url))")
            return reply(["error": "NotAllowedError", "message": "the embedder declined to leave the app for this page"])
        }
        #if os(iOS)
        if shown != nil { return reply(["error": "InvalidStateError", "message": "a page is already shown"]) }
        guard let top = top() else { return reply(["error": "InvalidStateError", "message": "no window to show the page in"]) }
        let apple = args["apple"] as? [String: Any] ?? [:]
        let configuration = SFSafariViewController.Configuration()
        if let reader = apple["entersReaderIfAvailable"] as? Bool { configuration.entersReaderIfAvailable = reader }
        if let collapsing = apple["barCollapsingEnabled"] as? Bool { configuration.barCollapsingEnabled = collapsing }
        let view = SFSafariViewController(url: url, configuration: configuration)
        switch apple["dismissButtonStyle"] as? String {
        case "close": view.dismissButtonStyle = .close
        case "cancel": view.dismissButtonStyle = .cancel
        default: break
        }
        Self.safariViewController?(view)
        view.delegate = self
        shown = (view, context.request, reply)
        say("opened \(Self.origin(url))")
        top.present(view, animated: true)
        #elseif os(macOS)
        NSWorkspace.shared.open(url)
        say("opened \(Self.origin(url))")
        reply(["action": "opened"])
        #endif
    }

    #if os(iOS)
    /// The session's window's topmost presented controller.
    private func top() -> UIViewController? {
        var top = context.presentation?.rootViewController
        while let presented = top?.presentedViewController { top = presented }
        return top
    }
    #endif

    public func cancel(_ request: Int) {
        #if os(iOS)
        guard let s = shown, s.request == request else { return }
        shown = nil
        s.view.dismiss(animated: true)
        s.reply(["error": "AbortError"])
        #endif
    }

    public func destroy(_ done: @escaping () -> Void) {
        #if os(iOS)
        if let s = shown { shown = nil; s.view.dismiss(animated: false); s.reply(["error": "AbortError"]) }
        #endif
        done()
    }
}

#if os(iOS)
extension InAppBrowserModule: SFSafariViewControllerDelegate {
    public func safariViewControllerDidFinish(_ controller: SFSafariViewController) {
        guard let s = shown, s.view === controller else { return }
        shown = nil
        say("dismissed")
        s.reply(["action": "dismissed"])
    }
}
#endif

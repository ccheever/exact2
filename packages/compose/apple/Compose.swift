// @exact/compose on iOS and macOS (LLP 1133.024 D14–D16): the system's
// composers, prefilled for the person to send.
//
// - iOS: `MFMailComposeViewController` when `canSendMail()`, its result the
//   answer; with no Mail account a `mailto:` URL through `UIApplication.open`,
//   answering `opened`, and `emailAttachments` false.
//   `MFMessageComposeViewController` when `canSendText()`, with a subject and
//   attachments where it says it takes them.
// - macOS: a mail with no attachment is a `mailto:` URL through `NSWorkspace`,
//   which keeps `cc` and `bcc`; one with attachments is
//   `NSSharingService(named: .composeEmail)`, which has one recipients list,
//   so a `cc` or `bcc` beside attachments refuses with `NotSupportedError`
//   (`emailCopiesWithAttachments` false). A message is `.composeMessage`,
//   which takes no subject. Both answer `opened`, the window being Mail's or
//   Messages's, or `failed`.
// - A hand-off by URL leaves the app, so the embedder decides first
//   (`context.leave`, D22 item 7).
// - A field the fact says this host cannot carry refuses with
//   `NotSupportedError` rather than being dropped.
// - Under the agent nothing opens: the `compose` ticket's tap is the answer.
// - The journal names the composer, the counts and the attachments' types,
//   never an address, the subject or the body.
import ExactPackage
import Foundation
import UniformTypeIdentifiers
#if os(iOS)
import MessageUI
import UIKit
#elseif os(macOS)
import AppKit
#endif

public final class ComposeModule: NSObject, ExactPackage {
    let context: ExactPackageContext
    #if os(iOS)
    private var shown: (view: UIViewController, request: Int?, kind: String, reply: ([String: Any]) -> Void)?
    #elseif os(macOS)
    private var sharing: (service: NSSharingService, kind: String, reply: ([String: Any]) -> Void)?
    #endif
    private var said: [String: Any]?

    public init(context: ExactPackageContext) { self.context = context }

    private func say(_ line: String) { context.log("compose: \(line)") }

    /// What this host can do now (D14): read again at each foreground, since a Mail account added in Settings changes it.
    var status: [String: Any] {
        // Under the agent the fact is the platform's shape, never this simulator's accounts: the drive sets it
        // (`--prefer compose.<field>=…`).
        #if os(iOS)
        if context.agent {
            return ["available": true, "email": true, "emailAttachments": true, "emailCopiesWithAttachments": true,
                    "message": true, "messageSubject": true, "messageAttachments": true]
        }
        let mail = MFMailComposeViewController.canSendMail(), text = MFMessageComposeViewController.canSendText()
        return ["available": true, "email": true, "emailAttachments": mail, "emailCopiesWithAttachments": mail,
                "message": text, "messageSubject": text && MFMessageComposeViewController.canSendSubject(),
                "messageAttachments": text && MFMessageComposeViewController.canSendAttachments()]
        #elseif os(macOS)
        let mail = NSSharingService(named: .composeEmail) != nil, message = NSSharingService(named: .composeMessage) != nil
        return ["available": true, "email": true, "emailAttachments": mail, "emailCopiesWithAttachments": false,
                "message": message, "messageSubject": false, "messageAttachments": message]
        #else
        return ["available": false, "email": false, "emailAttachments": false, "emailCopiesWithAttachments": false,
                "message": false, "messageSubject": false, "messageAttachments": false]
        #endif
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        let tap = (preferred as? [String: Any])?["tap"] as? String
        switch name {
        case "composeStatus":
            said = status
            reply(status)
        case "composeEmail": email(Fields(args, context), tap: tap, reply)
        case "composeMessage": message(Fields(args, context), tap: tap, reply)
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name.hasPrefix("composeStatus") ? status : ["error": "NotSupportedError"]
    }

    public func foreground(_ shown: Bool) {
        guard shown, let before = said, !NSDictionary(dictionary: before).isEqual(to: status) else { return }
        said = status
        context.changed("compose.composeStatus")
    }

    /// A send's fields, its attachments resolved to the files their admitted paths name.
    struct Fields {
        let to, cc, bcc: [String]
        let subject, body: String
        let attachments: [URL]
        let missing: Bool

        init(_ args: [String: Any], _ context: ExactPackageContext) {
            let list = { (key: String) in (args[key] as? [Any] ?? []).compactMap { $0 as? String }.filter { !$0.isEmpty } }
            (to, cc, bcc) = (list("to"), list("cc"), list("bcc"))
            subject = args["subject"] as? String ?? ""
            body = args["body"] as? String ?? ""
            let paths = list("attachments"), files = paths.compactMap { context.file($0) }
            attachments = files
            missing = files.count != paths.count
        }

        /// The journal's line: counts and types, never an address, the subject or the body.
        func line(_ composer: String, _ action: String) -> String {
            let types = attachments.map { UTType(filenameExtension: $0.pathExtension)?.preferredMIMEType ?? "application/octet-stream" }
            return "\(composer) \(action), \(to.count + cc.count + bcc.count) recipients, \(attachments.count) attachments" + (types.isEmpty ? "" : " (\(types.joined(separator: ", ")))")
        }

        /// RFC 6068 §2's `mailto:` URL.
        var mailto: URL? {
            var c = URLComponents()
            c.scheme = "mailto"
            c.path = to.joined(separator: ",")
            let headers = [("cc", cc.joined(separator: ",")), ("bcc", bcc.joined(separator: ",")), ("subject", subject), ("body", body)].filter { !$0.1.isEmpty }
            if !headers.isEmpty { c.queryItems = headers.map { URLQueryItem(name: $0.0, value: $0.1) } }
            return c.url
        }
    }

    private func refuse(_ message: String, _ reply: ([String: Any]) -> Void) {
        reply(["error": "NotSupportedError", "message": message])
    }

    private func email(_ f: Fields, tap: String?, _ reply: @escaping ([String: Any]) -> Void) {
        if f.missing { return reply(["error": "NotFoundError", "message": "an attachment names no app file"]) }
        let now = status
        if !f.attachments.isEmpty && now["emailAttachments"] as? Bool != true { return refuse("this host carries no attachment in a mail now; check composeStatus().emailAttachments", reply) }
        if !f.attachments.isEmpty && !(f.cc.isEmpty && f.bcc.isEmpty) && now["emailCopiesWithAttachments"] as? Bool != true {
            return refuse("this host's composer keeps no cc or bcc beside attachments; check composeStatus().emailCopiesWithAttachments", reply)
        }
        if context.agent {
            let action = tap ?? "sent"
            say(f.line("email", action))
            return reply(["action": action])
        }
        #if os(iOS)
        if MFMailComposeViewController.canSendMail() {
            if shown != nil { return reply(["error": "InvalidStateError", "message": "a composer is already shown"]) }
            guard let top = top() else { return reply(["error": "InvalidStateError", "message": "no window to show the composer in"]) }
            let request = context.request
            // Each attachment is read off the main thread, then the composer shows.
            DispatchQueue.global(qos: .userInitiated).async {
                let data = f.attachments.map { (url: $0, data: try? Data(contentsOf: $0)) }
                DispatchQueue.main.async { [self] in
                    if data.contains(where: { $0.data == nil }) { return reply(["error": "NotReadableError", "message": "an attachment could not be read"]) }
                    let view = MFMailComposeViewController()
                    view.mailComposeDelegate = self
                    view.setToRecipients(f.to)
                    view.setCcRecipients(f.cc)
                    view.setBccRecipients(f.bcc)
                    view.setSubject(f.subject)
                    view.setMessageBody(f.body, isHTML: false)
                    for a in data {
                        let type = UTType(filenameExtension: a.url.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
                        view.addAttachmentData(a.data!, mimeType: type, fileName: a.url.lastPathComponent)
                    }
                    shown = (view, request, "email", reply)
                    say(f.line("email", "shown"))
                    top.present(view, animated: true)
                }
            }
            return
        }
        #elseif os(macOS)
        if !f.attachments.isEmpty {
            return share(.composeEmail, kind: "email", recipients: f.to, subject: f.subject, items: [f.body as NSString].filter { $0.length > 0 } + f.attachments.map { $0 as NSURL }, f, reply)
        }
        #endif
        guard let url = f.mailto else { return reply(["error": "TypeError", "message": "the fields make no mailto: URL"]) }
        open(url, "email", f, reply)
    }

    private func message(_ f: Fields, tap: String?, _ reply: @escaping ([String: Any]) -> Void) {
        if f.missing { return reply(["error": "NotFoundError", "message": "an attachment names no app file"]) }
        let now = status
        if now["message"] as? Bool != true { return refuse("this host has no message composer now; check composeStatus().message", reply) }
        if !f.subject.isEmpty && now["messageSubject"] as? Bool != true { return refuse("this host's message composer takes no subject; check composeStatus().messageSubject", reply) }
        if !f.attachments.isEmpty && now["messageAttachments"] as? Bool != true { return refuse("this host's message composer takes no attachment; check composeStatus().messageAttachments", reply) }
        if context.agent {
            let action = tap ?? "sent"
            say(f.line("message", action))
            return reply(["action": action])
        }
        #if os(iOS)
        if shown != nil { return reply(["error": "InvalidStateError", "message": "a composer is already shown"]) }
        guard let top = top() else { return reply(["error": "InvalidStateError", "message": "no window to show the composer in"]) }
        let view = MFMessageComposeViewController()
        view.messageComposeDelegate = self
        view.recipients = f.to
        view.body = f.body
        if !f.subject.isEmpty { view.subject = f.subject }
        for url in f.attachments { view.addAttachmentURL(url, withAlternateFilename: url.lastPathComponent) }
        shown = (view, context.request, "message", reply)
        say(f.line("message", "shown"))
        top.present(view, animated: true)
        #elseif os(macOS)
        share(.composeMessage, kind: "message", recipients: f.to, subject: nil, items: [f.body as NSString].filter { $0.length > 0 } + f.attachments.map { $0 as NSURL }, f, reply)
        #endif
    }

    /// A hand-off by URL: the embedder first, then the system opens it.
    private func open(_ url: URL, _ kind: String, _ f: Fields, _ reply: @escaping ([String: Any]) -> Void) {
        guard context.leave(url.absoluteString, how: "external") else {
            say("the embedder kept the app from opening the \(kind) composer")
            return reply(["error": "NotAllowedError", "message": "the embedder declined to leave the app for the composer"])
        }
        #if os(iOS)
        UIApplication.shared.open(url) { [self] opened in
            say(f.line(kind, opened ? "opened" : "failed"))
            reply(["action": opened ? "opened" : "failed"])
        }
        #elseif os(macOS)
        let opened = NSWorkspace.shared.open(url)
        say(f.line(kind, opened ? "opened" : "failed"))
        reply(["action": opened ? "opened" : "failed"])
        #endif
    }

    #if os(iOS)
    /// The session's window's topmost presented controller.
    private func top() -> UIViewController? {
        var top = context.presentation?.rootViewController
        while let presented = top?.presentedViewController { top = presented }
        return top
    }

    private func finish(_ controller: UIViewController, _ action: String) {
        guard let s = shown, s.view === controller else { return }
        shown = nil
        controller.dismiss(animated: true)
        say("\(s.kind) \(action)")
        s.reply(["action": action])
    }
    #endif

    #if os(macOS)
    private func share(_ name: NSSharingService.Name, kind: String, recipients: [String], subject: String?, items: [Any], _ f: Fields, _ reply: @escaping ([String: Any]) -> Void) {
        guard let service = NSSharingService(named: name), service.canPerform(withItems: items) else {
            return refuse("no app on this Mac composes this \(kind)", reply)
        }
        if sharing != nil { return reply(["error": "InvalidStateError", "message": "a composer is already being opened"]) }
        service.recipients = recipients
        if let subject, !subject.isEmpty { service.subject = subject }
        service.delegate = self
        sharing = (service, kind, reply)
        say(f.line(kind, "shown"))
        service.perform(withItems: items)
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
extension ComposeModule: MFMailComposeViewControllerDelegate, MFMessageComposeViewControllerDelegate {
    public func mailComposeController(_ controller: MFMailComposeViewController, didFinishWith result: MFMailComposeResult, error: Error?) {
        finish(controller, [.sent: "sent", .saved: "saved", .cancelled: "cancelled"][result] ?? "failed")
    }

    public func messageComposeViewController(_ controller: MFMessageComposeViewController, didFinishWith result: MessageComposeResult) {
        finish(controller, [.sent: "sent", .cancelled: "cancelled"][result] ?? "failed")
    }
}
#elseif os(macOS)
extension ComposeModule: NSSharingServiceDelegate {
    public func sharingService(_ sharingService: NSSharingService, didShareItems items: [Any]) {
        guard let s = sharing, s.service === sharingService else { return }
        sharing = nil
        say("\(s.kind) opened")
        s.reply(["action": "opened"])
    }

    public func sharingService(_ sharingService: NSSharingService, didFailToShareItems items: [Any], error: Error) {
        guard let s = sharing, s.service === sharingService else { return }
        sharing = nil
        say("\(s.kind) failed")
        s.reply(["action": (error as NSError).code == NSUserCancelledError ? "cancelled" : "failed"])
    }
}
#endif

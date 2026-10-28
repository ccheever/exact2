// The tool door (LLP 1133.023 D7, D15 item 4): what a system's call of a
// tool reaches. The generated intents module (host/apple/intents.mjs) posts
// `exact.tools.call` `{tool, args, door, digest, reply}`, an
// `NSMutableDictionary` this marks `taken`, and an app's own native intent
// calls `ExactTools.call` directly. Either way the call is handed to the
// session's runner, which runs it as an invocation of origin `tool`
// (runner/src/runner/modules/calls.rs), and the door polls it by id until it
// ends.
//
// A call that launched the app with no scene boots the session with none,
// as a wake does (LLP 1133.017 D7): `boot`, which the app's host sets.
import Foundation
import os
#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

public enum ExactTools {
    /// Boots a session with no view for a call that launched the app with
    /// none; the app's host sets it. Nil: the call waits for a session.
    public static var boot: (() -> Void)?
    /// How long a call waits for its session to boot, and then for its end.
    private static let bootBound: Double = 20, endBound: Double = 15
    private static var installed = false
    static let logger = Logger(subsystem: "dev.exact", category: "tools")

    /// Hear the generated module's calls. Install at launch, before the app
    /// finishes launching: the system may call as soon as it has.
    public static func install() {
        guard !installed else { return }
        installed = true
        NotificationCenter.default.addObserver(forName: Notification.Name("exact.tools.call"), object: nil, queue: nil) { note in
            guard let body = note.object as? NSMutableDictionary, let tool = body["tool"] as? String,
                  let reply = body["reply"] as? ([String: Any]) -> Void else { return }
            body["taken"] = true
            let args = body["args"] as? [String: Any] ?? [:]
            let door = body["door"] as? String ?? "apple", digest = body["digest"] as? String
            DispatchQueue.main.async { call(tool: tool, args: args, door: door, digest: digest, reply: reply) }
        }
        ExactContentIndex.install()
    }

    /// Call `tool` with `args` as `door` hands it, and answer LLP 1111 D4's
    /// envelope: `outcome` (`ok`, `failed`, `superseded`, `timeout`,
    /// `refused`), `certainty`, `reason`, `value` and `message`. `digest` is
    /// the face's the caller was compiled with (D8), checked against the
    /// plan's. Main thread.
    public static func call(tool: String, args: [String: Any], door: String = "apple", digest: String? = nil,
                            reply: @escaping ([String: Any]) -> Void) {
        let started = Date()
        let refuse = { (reason: String, message: String) in
            reply(["outcome": "refused", "certainty": "none", "reason": reason, "message": message])
        }
        guard let session = ExactApp.shared.sessions.first(where: { $0.state != .destroyed }) else {
            return refuse("unavailable", "The app has no session to call.")
        }
        let cold = !session.booted || session.firstDrawMs == nil
        #if os(iOS)
        if cold, UIApplication.shared.applicationState == .background {
            log(session, "tools: \(tool) found no session ready; booting one with no view")
            boot?()
        }
        #endif
        func hand() {
            guard session.state != .destroyed else { return refuse("unavailable", "The app's session ended.") }
            guard session.booted else {
                if Date().timeIntervalSince(started) > bootBound { return refuse("busy", "The app did not start in time.") }
                return DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: hand)
            }
            var request: [String: Any] = ["op": "call", "tool": tool, "args": args, "door": door, "cold": cold, "hidden": hidden(session)]
            if let digest { request["digest"] = digest }
            let answer = json(session.agent(text(request)))
            guard let id = answer["call"] as? Int else {
                return refuse("unavailable", answer["error"] as? String ?? "The app refused the call.")
            }
            let handed = Date()
            func poll() {
                let polled = json(session.agent(text(["op": "called", "id": id])))
                if polled["outcome"] != nil { return reply(polled) }
                if polled["error"] != nil || Date().timeIntervalSince(handed) > endBound {
                    return reply(["outcome": "timeout", "certainty": "uncertain", "reason": "",
                                  "message": "It may or may not have taken effect: read before calling again."])
                }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: poll)
            }
            poll()
        }
        hand()
    }

    /// No view of the session is visible: its timers stop while the call runs (D7).
    private static func hidden(_ session: ExactSession) -> Bool {
        #if os(iOS)
        return UIApplication.shared.applicationState == .background
        #elseif os(macOS)
        return !NSApp.windows.contains { $0.isVisible && $0.occlusionState.contains(.visible) }
        #else
        return false
        #endif
    }

    static func log(_ session: ExactSession?, _ line: String) {
        session?.log(line)
        logger.log("\(line, privacy: .public)")
    }

    private static func text(_ object: [String: Any]) -> String {
        (try? JSONSerialization.data(withJSONObject: object)).map { String(decoding: $0, as: UTF8.self) } ?? "{}"
    }
    private static func json(_ text: String) -> [String: Any] {
        (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any] ?? ["error": "unreadable reply"]
    }
}

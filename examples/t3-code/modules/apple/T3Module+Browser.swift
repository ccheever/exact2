// T3Module's Browser ops (T3Module.swift routes them; browser-surface part 1): the data module's live sessions,
// a tab's navigation and its chrome row's commands, answered by the page registry (T3BrowserSessions.swift).
// Part 5: the previewAutomation host's plans (`browserAutomation`, T3BrowserAutomation.swift), which answer
// `previewAutomation.respond` on the focused connection, and a tab's Mute (`browserMute`).
import Foundation

extension T3Module: T3BrowserSessionOwner {
    func browserOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if browserSessions.performProfiles(request, reply: reply) { return } // part 4: clearing, the cookie import (T3BrowserSessions+Profiles.swift)
        let generation = request["generation"] as? Int ?? 0
        switch request["op"] as? String {
        case "browserAutomation":
            let plan = request["plan"] as? [String: Any] ?? [:]
            return DispatchQueue.main.async { [browserAutomation] in reply.send(["ok": true, "generation": generation, "value": browserAutomation.perform(plan)]) }
        case "browserMute":
            let tab = request["tab"] as? String ?? "", muted = request["muted"] as? Bool == true
            return DispatchQueue.main.async { [browserAutomation] in reply.send(["ok": true, "generation": generation, "value": ["done": browserAutomation.setMuted(tab, muted)]]) }
        default:
            if let answer = browserSessions.perform(request) ?? browserSessions.performNavigation(request) { return reply.send(answer) } // part 2: browserSet
            if browserSessions.performCapture(request, reply: { reply.send($0) }) { return } // part 3 (T3BrowserCapture.swift)
            next()
        }
    }

    /// The executor's pages and its connection: each new page gets the host's scripts; a response goes out on the
    /// focused transport under the generation the request came in on.
    func attachBrowserAutomation() {
        browserSessions.created = { [weak browserAutomation] session in browserAutomation?.prepare(session) }
        browserSessions.ended = { [weak browserAutomation] id in browserAutomation?.forget(id) }
        browserAutomation.changed = { [weak browserSessions] in browserSessions?.publish() }
        browserAutomation.rpc = { [weak transport, weak fleet] method, payload, generation, key, done in
            let request: [String: Any] = ["op": "request", "method": method, "payload": payload, "generation": generation]
            if let key { guard let fleet else { return done(["ok": false]) }; return fleet.perform(key, request, completion: done) }
            guard let transport else { return done(["ok": false]) }
            transport.perform(request, completion: done)
        }
    }
}
